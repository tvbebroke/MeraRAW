"""MeraNoise v1 — compact ISO-conditioned residual UNet for 576² ONNX tiles."""

from __future__ import annotations

import torch
import torch.nn as nn
import torch.nn.functional as F


class ConvBlock(nn.Module):
    def __init__(self, in_ch: int, out_ch: int) -> None:
        super().__init__()
        self.net = nn.Sequential(
            nn.Conv2d(in_ch, out_ch, 3, padding=1),
            nn.GroupNorm(min(8, out_ch), out_ch),
            nn.GELU(),
            nn.Conv2d(out_ch, out_ch, 3, padding=1),
            nn.GroupNorm(min(8, out_ch), out_ch),
            nn.GELU(),
        )

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        return self.net(x)


class MeraNoiseV1(nn.Module):
    """
    UNet denoiser: predicts display-gamma clean RGB from noisy RGB + noise map.
    Residual form: output = input_rgb + delta (stabilizes training).
    """

    def __init__(
        self,
        in_channels: int = 4,
        base_channels: int = 48,
        depth: int = 4,
    ) -> None:
        super().__init__()
        ch = base_channels
        self.in_conv = ConvBlock(in_channels, ch)
        self.down_samples = nn.ModuleList()
        self.down_blocks = nn.ModuleList()
        channels = [ch]
        c = ch
        for _ in range(depth):
            self.down_samples.append(nn.MaxPool2d(2))
            self.down_blocks.append(ConvBlock(c, c * 2))
            c *= 2
            channels.append(c)
        self.bottleneck = ConvBlock(c, c)
        self.up_trans = nn.ModuleList()
        self.up_blocks = nn.ModuleList()
        for i in range(depth):
            c_in = channels[-(i + 1)]
            c_skip = channels[-(i + 2)]
            self.up_trans.append(nn.ConvTranspose2d(c_in, c_skip, 2, stride=2))
            self.up_blocks.append(ConvBlock(c_skip * 2, c_skip))
        self.out_conv = nn.Conv2d(ch, 3, 1)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        noisy_rgb = x[:, :3]
        h = self.in_conv(x)
        skips: list[torch.Tensor] = []
        for down_sample, down_block in zip(self.down_samples, self.down_blocks):
            skips.append(h)
            h = down_block(down_sample(h))
        h = self.bottleneck(h)
        for up_t, up_b in zip(self.up_trans, self.up_blocks):
            h = up_t(h)
            skip = skips.pop()
            if h.shape[-2:] != skip.shape[-2:]:
                h = F.interpolate(h, size=skip.shape[-2:], mode="bilinear", align_corners=False)
            h = up_b(torch.cat([h, skip], dim=1))
        delta = self.out_conv(h)
        return noisy_rgb + delta


def build_model(cfg: dict) -> MeraNoiseV1:
    m = cfg.get("model", cfg)
    return MeraNoiseV1(
        in_channels=int(m.get("in_channels", 4)),
        base_channels=int(m.get("base_channels", 48)),
        depth=int(m.get("depth", 4)),
    )
