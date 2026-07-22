# MeraRAW External Contributor Onboarding

## Context

MeraRAW is a desktop RAW photo editor currently under development.

Core goals:

- Native RAW image support
- Non-destructive editing
- AI-powered editing assistant
- Professional photography workflow
- Lightroom alternative
- Local-first architecture

Current development is focused on the photo editor itself.

A separate project is being built to improve developer operations and bug reporting through Discord.

---

# Contributor

You are being brought in to build:

- Discord Bot
- Discord MCP integration
- Bug reporting workflow
- Automated markdown generation
- Developer notifications
- IDE integration layer

You are NOT currently being brought in to work on:

- RAW image processing
- Demosaicing engine
- Image rendering
- AI editing engine
- Proprietary editing algorithms
- Core photo editor source code

---

# Goal

Create a Discord-based bug reporting system that allows:

1. User submits bug report
2. Bot gathers structured information
3. Bot asks follow-up questions
4. Bot generates complete markdown issue
5. Markdown can be pasted directly into Cursor
6. Developer can immediately reproduce issue

Current process is manual.

Desired process is fully automated.

---

# Architecture

## User

↓

Discord Thread

↓

Bot Intake

↓

Question Flow

↓

Markdown Generation

↓

Developer Review

↓

Cursor

↓

Fix

---

# Desired Workflow

## Step 1

User creates bug report.

Example:

"Export crashes when using Sony ARW files."

---

## Step 2

Bot automatically asks:

### Environment

- Operating System
- OS Version
- MeraRAW Version

### Hardware

- CPU
- RAM
- GPU

### File Information

- Camera Model
- File Type
- File Size

### Reproduction

- What happened?
- Expected behavior?
- Actual behavior?
- Steps to reproduce?

### Attachments

- Screenshot
- Screen recording
- Log files

---

## Step 3

Bot compiles answers.

---

## Step 4

Bot generates markdown.

Example:

```md
# Bug Report

## Summary

Export crashes when using Sony ARW files.

## Environment

OS: Windows 11
CPU: Ryzen 7 7800X3D
GPU: RTX 4070

## Steps To Reproduce

1. Open ARW file
2. Apply exposure adjustment
3. Export JPEG
4. Crash

## Expected Result

JPEG exported successfully.

## Actual Result

Application crashes.

## Attachments

screenshot.png
video.mp4
```

---

## Step 5

Developer pastes markdown directly into Cursor.

---

# Discord Requirements

## Channel Structure

### Bugs

New bug reports are created as threads.

Example:

#bugs

└── Export crash on ARW files

---

### Feature Requests

Separate channel.

---

### Feedback

Separate channel.

---

# Thread Lifecycle

When issue is completed:

1. Moderator locks thread
2. No new messages allowed
3. Thread auto archives

Current Discord functionality already supports this.

---

# Access Restrictions

Contributor currently does NOT need:

- Full MeraRAW source code
- Core editor implementation
- Proprietary algorithms
- Internal AI systems

Contributor should be able to build the Discord bot independently.

---

# Recommended Repository Structure

github.com/org

├── meraraw
├── meraraw-discord-bot
├── meraraw-docs

Contributor should initially receive access only to:

- meraraw-discord-bot
- meraraw-docs

Not the primary editor repository.

---

# Future Integrations

Potential future integrations:

- GitHub Issues
- Linear
- Jira
- Notion
- Supabase
- Cursor
- Claude Code
- MCP servers

Design architecture so integrations can be added later.

---

# Technical Preferences

Preferred Stack:

- TypeScript
- Discord.js
- Node.js

Optional:

- PostgreSQL
- Supabase
- Redis

---

# Deliverables

## Phase 1

Working Discord bot that:

- Creates bug intake flow
- Collects structured information
- Generates markdown

---

## Phase 2

Developer workflow improvements:

- Templates
- Auto tagging
- Severity levels
- Priority levels

---

## Phase 3

Advanced integrations:

- GitHub sync
- MCP integration
- Cursor automation

---

# Intellectual Property

All code, documentation, workflows, designs, prompts, and deliverables created for MeraRAW are considered work product for MeraRAW and its parent company.

Contributor should not distribute proprietary materials outside the project.

Repository access should remain private.

---

# Current Decision

At this stage, contributor should build the Discord bot in a separate repository.

The primary MeraRAW source repository should remain isolated until there is a demonstrated need for deeper integration.
