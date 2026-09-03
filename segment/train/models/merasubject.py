# Compact U²-Net-style subject / saliency net for MeraRAW export.
# Intentionally small so M4 MPS + tract-onnx stay practical.

from __future__ import annotations

import torch
import torch.nn as nn
import torch.nn.functional as F


class ConvBNReLU(nn.Module):
    def __init__(self, cin: int, cout: int, k: int = 3):
        super().__init__()
        p = k // 2
        self.net = nn.Sequential(
            nn.Conv2d(cin, cout, k, padding=p, bias=False),
            nn.BatchNorm2d(cout),
            nn.ReLU(inplace=True),
        )

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        return self.net(x)


class MeraSubjectNet(nn.Module):
    """Encoder–decoder saliency @ 320², single-channel sigmoid mask."""

    def __init__(self, base: int = 32):
        super().__init__()
        b = base
        self.enc1 = nn.Sequential(ConvBNReLU(3, b), ConvBNReLU(b, b))
        self.enc2 = nn.Sequential(nn.MaxPool2d(2), ConvBNReLU(b, b * 2), ConvBNReLU(b * 2, b * 2))
        self.enc3 = nn.Sequential(
            nn.MaxPool2d(2), ConvBNReLU(b * 2, b * 4), ConvBNReLU(b * 4, b * 4)
        )
        self.enc4 = nn.Sequential(
            nn.MaxPool2d(2), ConvBNReLU(b * 4, b * 8), ConvBNReLU(b * 8, b * 8)
        )
        self.bridge = nn.Sequential(
            nn.MaxPool2d(2), ConvBNReLU(b * 8, b * 8), ConvBNReLU(b * 8, b * 8)
        )
        self.up4 = nn.ConvTranspose2d(b * 8, b * 8, 2, stride=2)
        self.dec4 = nn.Sequential(ConvBNReLU(b * 16, b * 8), ConvBNReLU(b * 8, b * 4))
        self.up3 = nn.ConvTranspose2d(b * 4, b * 4, 2, stride=2)
        self.dec3 = nn.Sequential(ConvBNReLU(b * 8, b * 4), ConvBNReLU(b * 4, b * 2))
        self.up2 = nn.ConvTranspose2d(b * 2, b * 2, 2, stride=2)
        self.dec2 = nn.Sequential(ConvBNReLU(b * 4, b * 2), ConvBNReLU(b * 2, b))
        self.up1 = nn.ConvTranspose2d(b, b, 2, stride=2)
        self.dec1 = nn.Sequential(ConvBNReLU(b * 2, b), ConvBNReLU(b, b))
        self.head = nn.Conv2d(b, 1, 1)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        e1 = self.enc1(x)
        e2 = self.enc2(e1)
        e3 = self.enc3(e2)
        e4 = self.enc4(e3)
        b = self.bridge(e4)
        d4 = self.dec4(torch.cat([self.up4(b), e4], dim=1))
        d3 = self.dec3(torch.cat([self.up3(d4), e3], dim=1))
        d2 = self.dec2(torch.cat([self.up2(d3), e2], dim=1))
        d1 = self.dec1(torch.cat([self.up1(d2), e1], dim=1))
        return torch.sigmoid(self.head(d1))


def soft_dice_bce(pred: torch.Tensor, target: torch.Tensor, eps: float = 1e-5) -> torch.Tensor:
    bce = F.binary_cross_entropy(pred, target)
    dims = (1, 2, 3)
    inter = (pred * target).sum(dims)
    den = pred.sum(dims) + target.sum(dims)
    dice = 1.0 - (2 * inter + eps) / (den + eps)
    return bce + dice.mean()
