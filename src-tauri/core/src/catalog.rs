//! Catalog — contract D2: a DISPOSABLE SQLite index over the filesystem.
//! Originals = truth; edits + metadata live in sidecars (D1) and are
//! mirrored here for speed. Delete this DB and `rebuild_index` restores
//! everything from folders + sidecars — the anti-corruption guarantee (R8).

use crate::error::CoreError;
use crate::raw::{Decoder, ImageMeta, RawlerDecoder};
use crate::sidecar;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

pub const CATALOG_SCHEMA_VERSION: i64 = 2;

fn db_err(e: impl std::fmt::Display) -> CoreError {
    CoreError::Io(format!("catalog: {e}"))
}

/// App data dir (`MERATECH_DATA_DIR` overrides for tests/dev).
pub fn data_dir() -> PathBuf {
    if let Ok(d) = std::env::var("MERATECH_DATA_DIR") {
        if !d.is_empty() {
            return PathBuf::from(d);
        }
    }
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        return PathBuf::from(home).join("Library/Application Support/MeraRAW");
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(base) = std::env::var("LOCALAPPDATA").or_else(|_| std::env::var("APPDATA")) {
            return PathBuf::from(base).join("MeraRAW");
        }
        let home = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into());
        return PathBuf::from(home)
            .join("AppData")
            .join("Local")
            .join("MeraRAW");
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            return PathBuf::from(xdg).join("MeraRAW");
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        PathBuf::from(home).join(".local/share/MeraRAW")
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderItem {
    pub root: String,
    /// Last path component for display (e.g. "2024-Graduation").
    pub name: String,
    pub photo_count: i64,
    /// False when the import root is missing (e.g. external drive unmounted).
    pub accessible: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GridItem {
    pub id: i64,
    pub path: String,
    pub filename: String,
    pub width: u32,
    pub height: u32,
    pub rating: u8,
    pub flag: String,
    pub label: Option<String>,
    pub has_edits: bool,
    pub captured_at: Option<String>,
    pub camera_model: Option<String>,
    pub blur_score: Option<f64>,
    pub has_thumb: bool,
    /// False when the original file is not reachable on disk.
    pub accessible: bool,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GridQuery {
    pub text: Option<String>,
    pub rating_min: Option<u8>,
    pub flag: Option<String>,
    pub label: Option<String>,
    pub camera: Option<String>,
    pub has_edits: Option<bool>,
    pub folder: Option<String>,
    pub blurry_only: bool,
    pub dupes_only: bool,
    pub sort: Option<String>, // "captured" (default) | "imported" | "rating"
    /// Filter to photos in a user album (see albums table).
    pub album_id: Option<i64>,
    pub offset: i64,
    pub limit: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidate {
    pub path: String,
    pub filename: String,
    /// True when the file is not yet in the catalog at the current mtime.
    pub is_new: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetDetail {
    pub id: i64,
    pub path: String,
    pub filename: String,
    pub width: u32,
    pub height: u32,
    pub rating: u8,
    pub flag: String,
    pub label: Option<String>,
    pub has_edits: bool,
    pub captured_at: Option<String>,
    pub imported_at: Option<String>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<i32>,
    pub shutter: Option<String>,
    pub aperture: Option<f64>,
    pub focal_mm: Option<f64>,
    pub keywords: Vec<String>,
    pub albums: Vec<String>,
    pub accessible: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumItem {
    pub id: i64,
    pub name: String,
    pub photo_count: i64,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MetaPatch {
    pub rating: Option<u8>,
    pub flag: Option<String>,
    pub label: Option<Option<String>>,
    pub add_keyword: Option<String>,
    pub remove_keyword: Option<String>,
}

fn is_allowed_flag(flag: &str) -> bool {
    matches!(flag, "none" | "pick" | "reject")
}

/// Strip trailing `/` or `\` so Windows and Unix roots compare cleanly.
pub fn trim_path_root(root: &str) -> &str {
    root.trim_end_matches(['/', '\\'])
}

/// `(root, like_forward, like_backslash)` for SQL
/// `path = root OR path LIKE root/% OR path LIKE root\%`.
///
/// Windows stores `C:\photos\IMG.ARW`; a Unix-only `root/%` LIKE never matches.
fn path_under_root_patterns(root: &str) -> (String, String, String) {
    let root = trim_path_root(root).to_string();
    let like_fwd = format!("{root}/%");
    let like_back = format!("{root}\\%");
    (root, like_fwd, like_back)
}

/// SQL fragment + bound params: asset is the folder root or anywhere under it.
fn sql_under_folder(root: &str) -> (String, [String; 3]) {
    let (root, like_fwd, like_back) = path_under_root_patterns(root);
    (
        "(path = ? OR path LIKE ? OR path LIKE ? OR folder = ? OR folder LIKE ? OR folder LIKE ?)"
            .into(),
        [root, like_fwd, like_back],
    )
}

pub struct Catalog {
    conn: Connection,
    dir: PathBuf,
}

impl Catalog {
    pub fn open_default() -> Result<Self, CoreError> {
        Self::open_at(data_dir())
    }

    pub fn open_at(dir: PathBuf) -> Result<Self, CoreError> {
        std::fs::create_dir_all(dir.join("previews"))?;
        let conn = Connection::open(dir.join("catalog.db")).map_err(db_err)?;
        conn.pragma_update(None, "journal_mode", "WAL").map_err(db_err)?;
        conn.pragma_update(None, "synchronous", "NORMAL").map_err(db_err)?;
        conn.pragma_update(None, "foreign_keys", "ON").map_err(db_err)?;
        let cat = Self { conn, dir };
        cat.init_schema()?;
        Ok(cat)
    }

    fn init_schema(&self) -> Result<(), CoreError> {
        self.conn
            .execute_batch(
                r#"
        CREATE TABLE IF NOT EXISTS meta(key TEXT PRIMARY KEY, value TEXT);
        CREATE TABLE IF NOT EXISTS assets(
            id INTEGER PRIMARY KEY,
            path TEXT UNIQUE NOT NULL,
            folder TEXT NOT NULL,
            filename TEXT NOT NULL,
            partial_hash TEXT,
            size INTEGER, modified_ms INTEGER,
            camera_make TEXT, camera_model TEXT, lens TEXT,
            iso INTEGER, shutter TEXT, aperture REAL, focal_mm REAL,
            captured_at TEXT, width INTEGER, height INTEGER,
            rating INTEGER DEFAULT 0,
            flag TEXT DEFAULT 'none',
            label TEXT,
            has_edits INTEGER DEFAULT 0,
            phash TEXT, blur_score REAL,
            thumb INTEGER DEFAULT 0,
            imported_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_assets_folder ON assets(folder);
        CREATE INDEX IF NOT EXISTS idx_assets_rating ON assets(rating);
        CREATE INDEX IF NOT EXISTS idx_assets_captured ON assets(captured_at);
        CREATE INDEX IF NOT EXISTS idx_assets_phash ON assets(phash);
        CREATE TABLE IF NOT EXISTS keywords(
            asset_id INTEGER NOT NULL,
            keyword TEXT NOT NULL,
            PRIMARY KEY(asset_id, keyword)
        );
        CREATE TABLE IF NOT EXISTS folders(root TEXT PRIMARY KEY);
        CREATE TABLE IF NOT EXISTS albums(
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            created_at TEXT
        );
        CREATE TABLE IF NOT EXISTS album_assets(
            album_id INTEGER NOT NULL,
            asset_id INTEGER NOT NULL,
            PRIMARY KEY(album_id, asset_id)
        );
        CREATE INDEX IF NOT EXISTS idx_album_assets_asset ON album_assets(asset_id);
        "#,
            )
            .map_err(db_err)?;
        self.conn
            .execute(
                "INSERT OR IGNORE INTO meta(key, value) VALUES('schema_version', ?1)",
                [CATALOG_SCHEMA_VERSION.to_string()],
            )
            .map_err(db_err)?;
        Ok(())
    }

    /// Parse `facet:value` tokens out of the free-text field into structured filters.
    pub fn normalize_query(q: &mut GridQuery) {
        let Some(text) = q.text.take() else {
            return;
        };
        let mut free = Vec::new();
        for token in text.split_whitespace() {
            if let Some(rest) = token.strip_prefix("camera:") {
                if !rest.is_empty() {
                    q.camera = Some(rest.to_string());
                }
            } else if let Some(rest) = token.strip_prefix("rating:") {
                if let Ok(r) = rest.parse::<u8>() {
                    q.rating_min = Some(r.max(1));
                }
            } else if let Some(rest) = token.strip_prefix("flag:") {
                if !rest.is_empty() {
                    q.flag = Some(rest.to_string());
                }
            } else if let Some(rest) = token.strip_prefix("label:") {
                if !rest.is_empty() {
                    q.label = Some(rest.to_string());
                }
            } else if let Some(rest) = token.strip_prefix("keyword:") {
                if !rest.is_empty() {
                    free.push(rest.to_string());
                }
            } else {
                free.push(token.to_string());
            }
        }
        if free.is_empty() {
            q.text = None;
        } else {
            q.text = Some(free.join(" "));
        }
    }

    pub fn previews_dir(&self) -> PathBuf {
        self.dir.join("previews")
    }

    pub fn thumb_path(&self, id: i64) -> PathBuf {
        self.previews_dir().join(format!("{id}-t.jpg"))
    }

    pub fn preview_path(&self, id: i64) -> PathBuf {
        self.previews_dir().join(format!("{id}-p.jpg"))
    }

    /// Remember an import root (also mirrored to folders.json beside the DB
    /// so a deleted DB can still find its folders — filesystem is truth).
    pub fn remember_folder(&mut self, root: &str) -> Result<(), CoreError> {
        self.conn
            .execute("INSERT OR IGNORE INTO folders(root) VALUES(?1)", [root])
            .map_err(db_err)?;
        let roots = self.folders()?;
        std::fs::write(
            self.dir.join("folders.json"),
            serde_json::to_string_pretty(&roots).unwrap_or_default(),
        )?;
        Ok(())
    }

    pub fn folders(&self) -> Result<Vec<String>, CoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT root FROM folders ORDER BY root")
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(db_err)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Imported roots with counts and live volume availability.
    pub fn list_folders(&self) -> Result<Vec<FolderItem>, CoreError> {
        let mut out = Vec::new();
        for root in self.folders()? {
            let (clause, [root_trim, like_fwd, like_back]) = sql_under_folder(&root);
            let sql = format!("SELECT COUNT(*) FROM assets WHERE {clause}");
            let photo_count: i64 = self
                .conn
                .query_row(
                    &sql,
                    rusqlite::params![
                        root_trim,
                        like_fwd,
                        like_back,
                        root_trim,
                        like_fwd,
                        like_back
                    ],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            let accessible = Path::new(&root_trim).exists();
            let name = Path::new(&root_trim)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| root_trim.clone());
            out.push(FolderItem {
                root: root_trim,
                name,
                photo_count,
                accessible,
            });
        }
        Ok(out)
    }

    /// Roots recorded in folders.json (survives DB deletion).
    pub fn folders_from_manifest(dir: &Path) -> Vec<String> {
        std::fs::read_to_string(dir.join("folders.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Path already imported with the same mtime? (incremental skip)
    pub fn is_current(&self, path: &str, modified_ms: i64) -> bool {
        self.conn
            .query_row(
                "SELECT 1 FROM assets WHERE path = ?1 AND modified_ms = ?2",
                rusqlite::params![path, modified_ms],
                |_| Ok(()),
            )
            .is_ok()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn upsert_asset(
        &mut self,
        path: &str,
        folder: &str,
        partial_hash: &str,
        size: i64,
        modified_ms: i64,
        meta: &ImageMeta,
        sidecar_meta: Option<&crate::doc::DocMeta>,
        has_edits: bool,
        phash: Option<&str>,
        blur_score: Option<f64>,
    ) -> Result<i64, CoreError> {
        let filename = Path::new(path)
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        let now = chrono_like_now();
        self.conn
            .execute(
                r#"INSERT INTO assets(path, folder, filename, partial_hash, size,
                    modified_ms, camera_make, camera_model, lens, iso, shutter,
                    aperture, focal_mm, captured_at, width, height, rating, flag,
                    label, has_edits, phash, blur_score, imported_at)
                VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23)
                ON CONFLICT(path) DO UPDATE SET
                    partial_hash=?4, size=?5, modified_ms=?6, camera_make=?7,
                    camera_model=?8, lens=?9, iso=?10, shutter=?11, aperture=?12,
                    focal_mm=?13, captured_at=?14, width=?15, height=?16,
                    rating=?17, flag=?18, label=?19, has_edits=?20, phash=?21,
                    blur_score=?22"#,
                rusqlite::params![
                    path,
                    folder,
                    filename,
                    partial_hash,
                    size,
                    modified_ms,
                    meta.camera_make,
                    meta.camera_model,
                    meta.lens,
                    meta.iso,
                    meta.shutter,
                    meta.aperture,
                    meta.focal_mm,
                    meta.captured_at,
                    meta.width,
                    meta.height,
                    sidecar_meta.map(|m| m.rating).unwrap_or(0),
                    sidecar_meta
                        .map(|m| if m.flag.is_empty() { "none".into() } else { m.flag.clone() })
                        .unwrap_or_else(|| "none".into()),
                    sidecar_meta.and_then(|m| m.label.clone()),
                    has_edits as i64,
                    phash,
                    blur_score,
                    now,
                ],
            )
            .map_err(db_err)?;
        let id: i64 = self
            .conn
            .query_row("SELECT id FROM assets WHERE path = ?1", [path], |r| r.get(0))
            .map_err(db_err)?;
        if let Some(sm) = sidecar_meta {
            for kw in &sm.keywords {
                self.conn
                    .execute(
                        "INSERT OR IGNORE INTO keywords(asset_id, keyword) VALUES(?1, ?2)",
                        rusqlite::params![id, kw],
                    )
                    .map_err(db_err)?;
            }
        }
        Ok(id)
    }

    pub fn mark_thumb(&mut self, id: i64) -> Result<(), CoreError> {
        self.conn
            .execute("UPDATE assets SET thumb = 1 WHERE id = ?1", [id])
            .map_err(db_err)?;
        Ok(())
    }

    pub fn asset_path(&self, id: i64) -> Option<String> {
        self.conn
            .query_row("SELECT path FROM assets WHERE id = ?1", [id], |r| r.get(0))
            .ok()
    }

    /// Build a missing library thumb from the file's embedded preview (or a
    /// tiny decode). Used when older imports / rebuilds left `thumb=0`.
    pub fn ensure_thumb(&mut self, id: i64) -> Option<PathBuf> {
        let path = self.thumb_path(id);
        if path.exists() {
            let _ = self.mark_thumb(id);
            return Some(path);
        }
        let src = self.asset_path(id)?;
        let src = PathBuf::from(src);
        if !src.exists() {
            return None;
        }
        let dec = RawlerDecoder::default();
        let (rgba, w, h) = match dec.embedded_preview(&src, 640) {
            Ok(Some(v)) => v,
            _ => return None,
        };
        let img = image::RgbaImage::from_raw(w, h, rgba)?;
        let thumb = image::DynamicImage::ImageRgba8(img)
            .thumbnail(320, 320)
            .to_rgb8();
        let bytes = encode_jpeg(&thumb, 80)?;
        if std::fs::write(&path, bytes).is_err() {
            return None;
        }
        let _ = self.mark_thumb(id);
        Some(path)
    }

    pub fn count(&self) -> i64 {
        self.conn
            .query_row("SELECT COUNT(*) FROM assets", [], |r| r.get(0))
            .unwrap_or(0)
    }

    pub fn grid(&self, q: &GridQuery) -> Result<Vec<GridItem>, CoreError> {
        let mut q = q.clone();
        Self::normalize_query(&mut q);
        let mut sql = String::from(
            "SELECT id, path, filename, width, height, rating, flag, label,
                    has_edits, captured_at, camera_model, blur_score, thumb
             FROM assets WHERE 1=1",
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        if let Some(t) = q.text.as_ref().filter(|t| !t.is_empty()) {
            sql.push_str(
                " AND (filename LIKE ?  OR camera_model LIKE ? OR id IN
                   (SELECT asset_id FROM keywords WHERE keyword LIKE ?))",
            );
            let like = format!("%{t}%");
            params.push(Box::new(like.clone()));
            params.push(Box::new(like.clone()));
            params.push(Box::new(like));
        }
        if let Some(r) = q.rating_min.filter(|r| *r > 0) {
            sql.push_str(" AND rating >= ?");
            params.push(Box::new(r as i64));
        }
        if let Some(f) = q.flag.as_ref().filter(|f| !f.is_empty() && *f != "any") {
            sql.push_str(" AND flag = ?");
            params.push(Box::new(f.clone()));
        }
        if let Some(l) = q.label.as_ref().filter(|l| !l.is_empty()) {
            sql.push_str(" AND label = ?");
            params.push(Box::new(l.clone()));
        }
        if let Some(c) = q.camera.as_ref().filter(|c| !c.is_empty()) {
            sql.push_str(" AND camera_model LIKE ?");
            params.push(Box::new(format!("%{c}%")));
        }
        if let Some(h) = q.has_edits {
            sql.push_str(" AND has_edits = ?");
            params.push(Box::new(h as i64));
        }
        if let Some(f) = q.folder.as_ref().filter(|f| !f.is_empty()) {
            let (clause, [root, like_fwd, like_back]) = sql_under_folder(f);
            sql.push_str(" AND ");
            sql.push_str(&clause);
            params.push(Box::new(root.clone()));
            params.push(Box::new(like_fwd.clone()));
            params.push(Box::new(like_back.clone()));
            params.push(Box::new(root));
            params.push(Box::new(like_fwd));
            params.push(Box::new(like_back));
        }
        if q.blurry_only {
            sql.push_str(" AND blur_score IS NOT NULL AND blur_score < 35.0");
        }
        if q.dupes_only {
            sql.push_str(
                " AND phash IN (SELECT phash FROM assets WHERE phash IS NOT NULL
                                GROUP BY phash HAVING COUNT(*) > 1)",
            );
        }
        if let Some(album_id) = q.album_id.filter(|id| *id > 0) {
            sql.push_str(" AND id IN (SELECT asset_id FROM album_assets WHERE album_id = ?)");
            params.push(Box::new(album_id));
        }
        sql.push_str(match q.sort.as_deref() {
            Some("rating") => " ORDER BY rating DESC, captured_at",
            Some("imported") => " ORDER BY imported_at DESC",
            _ => " ORDER BY captured_at, filename",
        });
        sql.push_str(" LIMIT ? OFFSET ?");
        params.push(Box::new(if q.limit <= 0 { 500 } else { q.limit }));
        params.push(Box::new(q.offset.max(0)));

        let mut stmt = self.conn.prepare(&sql).map_err(db_err)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())), |r| {
                let path: String = r.get(1)?;
                Ok(GridItem {
                    id: r.get(0)?,
                    path: path.clone(),
                    filename: r.get(2)?,
                    width: r.get::<_, Option<u32>>(3)?.unwrap_or(0),
                    height: r.get::<_, Option<u32>>(4)?.unwrap_or(0),
                    rating: r.get::<_, i64>(5)? as u8,
                    flag: r.get(6)?,
                    label: r.get(7)?,
                    has_edits: r.get::<_, i64>(8)? != 0,
                    captured_at: r.get(9)?,
                    camera_model: r.get(10)?,
                    blur_score: r.get(11)?,
                    has_thumb: r.get::<_, i64>(12)? != 0,
                    accessible: Path::new(&path).exists(),
                })
            })
            .map_err(db_err)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn asset_detail(&self, id: i64) -> Result<Option<AssetDetail>, CoreError> {
        let row = self.conn.query_row(
            r#"SELECT id, path, filename, width, height, rating, flag, label,
                      has_edits, captured_at, imported_at, camera_make, camera_model,
                      lens, iso, shutter, aperture, focal_mm
               FROM assets WHERE id = ?1"#,
            [id],
            |r| {
                let path: String = r.get(1)?;
                Ok(AssetDetail {
                    id: r.get(0)?,
                    path: path.clone(),
                    filename: r.get(2)?,
                    width: r.get::<_, Option<u32>>(3)?.unwrap_or(0),
                    height: r.get::<_, Option<u32>>(4)?.unwrap_or(0),
                    rating: r.get::<_, i64>(5)? as u8,
                    flag: r.get(6)?,
                    label: r.get(7)?,
                    has_edits: r.get::<_, i64>(8)? != 0,
                    captured_at: r.get(9)?,
                    imported_at: r.get(10)?,
                    camera_make: r.get(11)?,
                    camera_model: r.get(12)?,
                    lens: r.get(13)?,
                    iso: r.get::<_, Option<i64>>(14)?.map(|v| v as i32),
                    shutter: r.get(15)?,
                    aperture: r.get(16)?,
                    focal_mm: r.get(17)?,
                    keywords: Vec::new(),
                    albums: Vec::new(),
                    accessible: Path::new(&path).exists(),
                })
            },
        );
        match row {
            Ok(mut d) => {
                d.keywords = self.keywords_of(id);
                d.albums = self.albums_for_asset(id);
                Ok(Some(d))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(db_err(e)),
        }
    }

    pub fn list_albums(&self) -> Result<Vec<AlbumItem>, CoreError> {
        let mut stmt = self
            .conn
            .prepare(
                r#"SELECT a.id, a.name,
                          (SELECT COUNT(*) FROM album_assets aa WHERE aa.album_id = a.id)
                   FROM albums a ORDER BY a.name"#,
            )
            .map_err(db_err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(AlbumItem {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    photo_count: r.get(2)?,
                })
            })
            .map_err(db_err)?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    pub fn create_album(&mut self, name: &str) -> Result<i64, CoreError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(CoreError::InvalidOp("album name required".into()));
        }
        self.conn
            .execute(
                "INSERT INTO albums(name, created_at) VALUES(?1, ?2)",
                rusqlite::params![name, chrono_like_now()],
            )
            .map_err(db_err)?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn delete_album(&mut self, id: i64) -> Result<(), CoreError> {
        self.conn
            .execute("DELETE FROM album_assets WHERE album_id = ?1", [id])
            .map_err(db_err)?;
        self.conn
            .execute("DELETE FROM albums WHERE id = ?1", [id])
            .map_err(db_err)?;
        Ok(())
    }

    pub fn add_to_album(&mut self, album_id: i64, asset_ids: &[i64]) -> Result<(), CoreError> {
        let album_ok: bool = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM albums WHERE id = ?1)",
                [album_id],
                |r| r.get(0),
            )
            .map_err(db_err)?;
        if !album_ok {
            return Err(CoreError::InvalidOp("album not found".into()));
        }
        for id in asset_ids {
            let asset_ok: bool = self
                .conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM assets WHERE id = ?1)",
                    [id],
                    |r| r.get(0),
                )
                .map_err(db_err)?;
            if !asset_ok {
                return Err(CoreError::InvalidOp(format!("asset not found: {id}")));
            }
            self.conn
                .execute(
                    "INSERT OR IGNORE INTO album_assets(album_id, asset_id) VALUES(?1, ?2)",
                    rusqlite::params![album_id, id],
                )
                .map_err(db_err)?;
        }
        Ok(())
    }

    pub fn remove_from_album(&mut self, album_id: i64, asset_ids: &[i64]) -> Result<(), CoreError> {
        for id in asset_ids {
            self.conn
                .execute(
                    "DELETE FROM album_assets WHERE album_id = ?1 AND asset_id = ?2",
                    rusqlite::params![album_id, id],
                )
                .map_err(db_err)?;
        }
        Ok(())
    }

    fn albums_for_asset(&self, asset_id: i64) -> Vec<String> {
        self.conn
            .prepare(
                r#"SELECT a.name FROM albums a
                   JOIN album_assets aa ON aa.album_id = a.id
                   WHERE aa.asset_id = ?1 ORDER BY a.name"#,
            )
            .and_then(|mut s| {
                let rows = s.query_map([asset_id], |r| r.get::<_, String>(0))?;
                Ok(rows.filter_map(|r| r.ok()).collect())
            })
            .unwrap_or_default()
    }

    /// Scan a folder for import candidates (does not import).
    pub fn scan_import_candidates(&self, root: &Path) -> Result<Vec<ImportCandidate>, CoreError> {
        if !root.is_dir() {
            return Err(CoreError::Io(format!("not a folder: {}", root.display())));
        }
        Ok(scan_folder(root)
            .into_iter()
            .map(|p| {
                let path = p.to_string_lossy().into_owned();
                let filename = p
                    .file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let modified_ms = std::fs::metadata(&p)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                let is_new = !self.is_current(&path, modified_ms);
                ImportCandidate {
                    path,
                    filename,
                    is_new,
                }
            })
            .collect())
    }

    /// Apply a metadata patch to assets: DB write + return (id, path) pairs
    /// so the engine mirrors the change into sidecars (portability, D1).
    pub fn set_meta(
        &mut self,
        ids: &[i64],
        patch: &MetaPatch,
    ) -> Result<Vec<(i64, String)>, CoreError> {
        let tx = self.conn.transaction().map_err(db_err)?;
        let mut touched = Vec::new();
        for id in ids {
            if let Some(r) = patch.rating {
                tx.execute(
                    "UPDATE assets SET rating = ?1 WHERE id = ?2",
                    rusqlite::params![r.min(5) as i64, id],
                )
                .map_err(db_err)?;
            }
            if let Some(f) = &patch.flag {
                if !is_allowed_flag(f) {
                    return Err(CoreError::InvalidOp(format!("invalid flag: {f}")));
                }
                tx.execute(
                    "UPDATE assets SET flag = ?1 WHERE id = ?2",
                    rusqlite::params![f, id],
                )
                .map_err(db_err)?;
            }
            if let Some(l) = &patch.label {
                tx.execute(
                    "UPDATE assets SET label = ?1 WHERE id = ?2",
                    rusqlite::params![l, id],
                )
                .map_err(db_err)?;
            }
            if let Some(kw) = &patch.add_keyword {
                tx.execute(
                    "INSERT OR IGNORE INTO keywords(asset_id, keyword) VALUES(?1, ?2)",
                    rusqlite::params![id, kw],
                )
                .map_err(db_err)?;
            }
            if let Some(kw) = &patch.remove_keyword {
                tx.execute(
                    "DELETE FROM keywords WHERE asset_id = ?1 AND keyword = ?2",
                    rusqlite::params![id, kw],
                )
                .map_err(db_err)?;
            }
            let path: String = tx
                .query_row("SELECT path FROM assets WHERE id = ?1", [id], |r| r.get(0))
                .map_err(db_err)?;
            touched.push((*id, path));
        }
        tx.commit().map_err(db_err)?;
        Ok(touched)
    }

    pub fn keywords_of(&self, id: i64) -> Vec<String> {
        self.conn
            .prepare("SELECT keyword FROM keywords WHERE asset_id = ?1 ORDER BY keyword")
            .and_then(|mut s| {
                let rows = s.query_map([id], |r| r.get::<_, String>(0))?;
                Ok(rows.filter_map(|r| r.ok()).collect())
            })
            .unwrap_or_default()
    }

    /// Nuke all rows (rebuild path). Previews stay (content-keyed by id is
    /// lost, but they regenerate lazily).
    pub fn wipe_assets(&mut self) -> Result<(), CoreError> {
        self.conn
            .execute_batch("DELETE FROM keywords; DELETE FROM album_assets; DELETE FROM assets;")
            .map_err(db_err)?;
        Ok(())
    }
}

/// One file's import work (worker-thread side; its own DB connection is
/// NOT used — results post back to the engine which owns the catalog).
pub struct ImportedFile {
    pub path: String,
    pub folder: String,
    pub partial_hash: String,
    pub size: i64,
    pub modified_ms: i64,
    pub meta: ImageMeta,
    pub sidecar_meta: Option<crate::doc::DocMeta>,
    pub has_edits: bool,
    pub phash: Option<String>,
    pub blur_score: Option<f64>,
    /// (thumb jpeg bytes, preview jpeg bytes) — engine writes them to the
    /// previews dir once the asset id is known.
    pub thumb_jpeg: Option<Vec<u8>>,
    pub preview_jpeg: Option<Vec<u8>>,
}

/// Scan a folder for RAW files (recursive).
pub fn scan_folder(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    let dec = RawlerDecoder::default();
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let name = p.file_name().map(|n| n.to_string_lossy().into_owned());
                if !name.map(|n| n.starts_with('.')).unwrap_or(true) {
                    stack.push(p);
                }
            } else if dec.probe(&p) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Process one file: metadata + sidecar + previews + cull signals.
/// Pure worker fn — no DB access (engine owns the catalog connection).
pub fn import_one(path: &Path) -> Result<ImportedFile, CoreError> {
    let dec = RawlerDecoder::default();
    let md = std::fs::metadata(path)?;
    let modified_ms = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);

    // partial hash: first 1MB + size (dedupe-grade, fast)
    let partial_hash = {
        use std::io::Read;
        let mut f = std::fs::File::open(path)?;
        let mut buf = vec![0u8; 1 << 20];
        let n = f.read(&mut buf)?;
        let mut h = blake3::Hasher::new();
        h.update(&buf[..n]);
        h.update(&md.len().to_le_bytes());
        h.finalize().to_hex().to_string()
    };

    let meta = dec.metadata(path)?;
    let doc = sidecar::load_sidecar(path).ok().flatten();
    let has_edits = doc.as_ref().map(|d| !d.modules.is_empty() || !d.masks.is_empty()).unwrap_or(false);
    let sidecar_meta = doc.map(|d| d.meta);

    // previews from the embedded JPEG (fast path)
    let (mut thumb_jpeg, mut preview_jpeg) = (None, None);
    let (mut phash, mut blur) = (None, None);
    if let Ok(Some((rgba, w, h))) = dec.embedded_preview(path, 1600) {
        if let Some(img) = image::RgbaImage::from_raw(w, h, rgba) {
            let dynimg = image::DynamicImage::ImageRgba8(img);
            let preview = dynimg.thumbnail(1600, 1600).to_rgb8();
            let thumb = dynimg.thumbnail(320, 320).to_rgb8();
            // cull signals from the thumb
            let gray = image::DynamicImage::ImageRgb8(thumb.clone()).to_luma8();
            phash = Some(average_hash(&gray));
            blur = Some(laplacian_variance(&gray));
            preview_jpeg = encode_jpeg(&preview, 86);
            thumb_jpeg = encode_jpeg(&thumb, 80);
        }
    }

    Ok(ImportedFile {
        path: path.to_string_lossy().into_owned(),
        folder: path
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default(),
        partial_hash,
        size: md.len() as i64,
        modified_ms,
        meta,
        sidecar_meta,
        has_edits,
        phash,
        blur_score: blur,
        thumb_jpeg,
        preview_jpeg,
    })
}

fn encode_jpeg(img: &image::RgbImage, quality: u8) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    enc.encode_image(img).ok()?;
    Some(out)
}

/// 8×8 average hash (dupe/burst grouping).
fn average_hash(gray: &image::GrayImage) -> String {
    let small = image::imageops::resize(gray, 8, 8, image::imageops::FilterType::Triangle);
    let mean: u32 = small.pixels().map(|p| p.0[0] as u32).sum::<u32>() / 64;
    let mut bits = 0u64;
    for (i, p) in small.pixels().enumerate() {
        if (p.0[0] as u32) > mean {
            bits |= 1 << i;
        }
    }
    format!("{bits:016x}")
}

/// Variance of a 3×3 Laplacian — classic focus/blur metric.
fn laplacian_variance(gray: &image::GrayImage) -> f64 {
    let (w, h) = gray.dimensions();
    if w < 3 || h < 3 {
        return 0.0;
    }
    let mut vals = Vec::with_capacity(((w - 2) * (h - 2)) as usize);
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let c = gray.get_pixel(x, y).0[0] as f64;
            let lap = -4.0 * c
                + gray.get_pixel(x - 1, y).0[0] as f64
                + gray.get_pixel(x + 1, y).0[0] as f64
                + gray.get_pixel(x, y - 1).0[0] as f64
                + gray.get_pixel(x, y + 1).0[0] as f64;
            vals.push(lap);
        }
    }
    let mean = vals.iter().sum::<f64>() / vals.len() as f64;
    vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / vals.len() as f64
}

fn chrono_like_now() -> String {
    // reuse the doc module's civil-date formatting indirectly: cheap copy
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::{EditDoc, ParamValue};

    fn meta_stub(path: &str) -> ImageMeta {
        ImageMeta {
            path: path.into(),
            kind: crate::raw::ImageKind::Raw,
            format: "ARW".into(),
            bit_depth: 0,
            camera_make: "Sony".into(),
            camera_model: "ILCE-7M3".into(),
            lens: None,
            iso: Some(200),
            shutter: Some("1/500".into()),
            aperture: Some(2.8),
            focal_mm: Some(85.0),
            captured_at: Some("2026:06:01 14:00:00".into()),
            width: 6000,
            height: 4000,
            orientation: "Normal".into(),
            as_shot_wb: [2.0, 1.0, 1.5],
            estimated_cct: Some(5200.0),
            camera_profile: None,
            available_profiles: Vec::new(),
            available_profile_files: Vec::new(),
            demosaic: String::new(),
            available_demosaic: Vec::new(),
        }
    }

    fn tmp_cat(name: &str) -> (Catalog, PathBuf) {
        let dir = std::env::temp_dir().join(format!("meratech-cat-{name}-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        (Catalog::open_at(dir.clone()).unwrap(), dir)
    }

    #[test]
    fn grid_folder_filter_matches_windows_backslash_paths() {
        let (mut cat, dir) = tmp_cat("win-paths");
        let root = r"C:\Users\test\photos";
        let p = format!(r"{root}\IMG_0001.ARW");
        let nested_folder = format!(r"{root}\trip");
        let nested = format!(r"{nested_folder}\IMG_0002.ARW");
        cat.upsert_asset(&p, root, "h", 100, 0, &meta_stub(&p), None, false, None, None)
            .unwrap();
        cat.upsert_asset(
            &nested,
            &nested_folder,
            "h",
            100,
            0,
            &meta_stub(&nested),
            None,
            false,
            None,
            None,
        )
        .unwrap();
        cat.remember_folder(root).unwrap();

        let grid = cat
            .grid(&GridQuery {
                folder: Some(root.into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(
            grid.len(),
            2,
            "folder filter must match Windows backslash paths"
        );

        let folders = cat.list_folders().unwrap();
        assert_eq!(folders.len(), 1);
        assert_eq!(folders[0].photo_count, 2);

        // Unix-style still works
        let (mut cat2, dir2) = tmp_cat("unix-paths");
        cat2.upsert_asset(
            "/photos/a/IMG_0001.ARW",
            "/photos/a",
            "h",
            100,
            0,
            &meta_stub("/photos/a/IMG_0001.ARW"),
            None,
            false,
            None,
            None,
        )
        .unwrap();
        let grid2 = cat2
            .grid(&GridQuery {
                folder: Some("/photos/a".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(grid2.len(), 1);

        std::fs::remove_dir_all(dir).ok();
        std::fs::remove_dir_all(dir2).ok();
    }

    #[test]
    fn upsert_grid_and_filters() {
        let (mut cat, dir) = tmp_cat("grid");
        for i in 0..5 {
            let p = format!("/photos/a/IMG_{i:04}.ARW");
            cat.upsert_asset(&p, "/photos/a", "h", 100, i, &meta_stub(&p), None, false, None, Some(if i == 0 { 10.0 } else { 200.0 }))
                .unwrap();
        }
        assert_eq!(cat.count(), 5);
        // re-upsert same path → no dupes (keep its blur signal)
        cat.upsert_asset("/photos/a/IMG_0000.ARW", "/photos/a", "h", 100, 0, &meta_stub("x"), None, false, None, Some(10.0))
            .unwrap();
        assert_eq!(cat.count(), 5);

        let all = cat.grid(&GridQuery::default()).unwrap();
        assert_eq!(all.len(), 5);

        cat.set_meta(&[all[0].id], &MetaPatch { rating: Some(4), ..Default::default() })
            .unwrap();
        let rated = cat
            .grid(&GridQuery { rating_min: Some(3), ..Default::default() })
            .unwrap();
        assert_eq!(rated.len(), 1);

        let blurry = cat
            .grid(&GridQuery { blurry_only: true, ..Default::default() })
            .unwrap();
        assert_eq!(blurry.len(), 1, "one asset has blur_score 10");

        let text = cat
            .grid(&GridQuery { text: Some("IMG_0003".into()), ..Default::default() })
            .unwrap();
        assert_eq!(text.len(), 1);

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn rebuild_from_sidecars_loses_nothing() {
        // the R8 anti-corruption guarantee, in miniature
        let dir = std::env::temp_dir().join(format!("meratech-rebuild-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let photos = dir.join("photos");
        std::fs::create_dir_all(&photos).unwrap();
        let img = photos.join("IMG_1.ARW");
        std::fs::write(&img, b"fake-raw").unwrap();

        // sidecar carries an edit + rating + keyword (truth on disk)
        let mut doc = EditDoc::new(img.to_str().unwrap());
        doc.set("exposure", "stops", ParamValue::F32(0.5));
        doc.meta.rating = 5;
        doc.meta.keywords = vec!["grad".into()];
        sidecar::write_sidecar(&img, &doc).unwrap();

        let catdir = dir.join("catalog");
        {
            let (mut cat, _) = (Catalog::open_at(catdir.clone()).unwrap(), ());
            let f = ImportedFile {
                path: img.to_string_lossy().into_owned(),
                folder: photos.to_string_lossy().into_owned(),
                partial_hash: "h".into(),
                size: 8,
                modified_ms: 1,
                meta: meta_stub(img.to_str().unwrap()),
                sidecar_meta: Some(doc.meta.clone()),
                has_edits: true,
                phash: None,
                blur_score: None,
                thumb_jpeg: None,
                preview_jpeg: None,
            };
            cat.upsert_asset(
                &f.path, &f.folder, &f.partial_hash, f.size, f.modified_ms, &f.meta,
                f.sidecar_meta.as_ref(), f.has_edits, None, None,
            )
            .unwrap();
            cat.remember_folder(&f.folder).unwrap();
            assert_eq!(cat.count(), 1);
        }

        // catastrophic loss: delete the DB entirely
        std::fs::remove_file(catdir.join("catalog.db")).unwrap();
        std::fs::remove_file(catdir.join("catalog.db-wal")).ok();
        std::fs::remove_file(catdir.join("catalog.db-shm")).ok();

        // rebuild: roots from the manifest, sidecars from disk
        let roots = Catalog::folders_from_manifest(&catdir);
        assert_eq!(roots.len(), 1, "folders.json must survive the DB");
        let mut cat = Catalog::open_at(catdir).unwrap();
        for root in &roots {
            for p in scan_folder(Path::new(root)) {
                // fake file won't decode; use the sidecar directly
                let doc = sidecar::load_sidecar(&p).unwrap().expect("sidecar");
                let f_meta = meta_stub(p.to_str().unwrap());
                cat.upsert_asset(
                    p.to_str().unwrap(),
                    root,
                    "h",
                    8,
                    1,
                    &f_meta,
                    Some(&doc.meta),
                    !doc.modules.is_empty(),
                    None,
                    None,
                )
                .unwrap();
            }
            cat.remember_folder(root).unwrap();
        }
        let items = cat.grid(&GridQuery::default()).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].rating, 5, "rating restored from sidecar");
        assert!(items[0].has_edits, "edits flag restored");
        assert_eq!(cat.keywords_of(items[0].id), vec!["grad".to_string()]);

        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn cull_signal_math() {
        // flat image → tiny laplacian variance; noisy → larger
        let flat = image::GrayImage::from_pixel(32, 32, image::Luma([128]));
        assert!(laplacian_variance(&flat) < 1.0);
        let mut noisy = flat.clone();
        for (i, p) in noisy.pixels_mut().enumerate() {
            p.0[0] = if i % 2 == 0 { 60 } else { 200 };
        }
        assert!(laplacian_variance(&noisy) > 100.0);
        // identical images → identical phash
        assert_eq!(average_hash(&flat), average_hash(&flat.clone()));
    }
}
