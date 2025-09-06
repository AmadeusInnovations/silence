#!/usr/bin/env python3
import subprocess
import os

# Create a simple SVG icon
svg_content = '''<?xml version="1.0" encoding="UTF-8"?>
<svg width="512" height="512" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <linearGradient id="bg" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" style="stop-color:#1e3c72;stop-opacity:1" />
      <stop offset="100%" style="stop-color:#2a5298;stop-opacity:1" />
    </linearGradient>
  </defs>
  
  <!-- Background circle -->
  <circle cx="256" cy="256" r="240" fill="url(#bg)" stroke="#ffffff" stroke-width="8"/>
  
  <!-- Lock icon -->
  <!-- Shackle -->
  <path d="M 180 180 A 76 76 0 0 1 332 180" stroke="white" stroke-width="16" fill="none"/>
  <path d="M 180 180 A 60 60 0 0 1 300 180" stroke="#1e3c72" stroke-width="8" fill="none"/>
  
  <!-- Body -->
  <rect x="150" y="220" width="212" height="140" rx="20" fill="white"/>
  <rect x="160" y="230" width="192" height="120" rx="15" fill="#1e3c72"/>
  
  <!-- Keyhole -->
  <circle cx="256" cy="270" r="18" fill="white"/>
  <rect x="250" y="280" width="12" height="30" fill="white"/>
  
  <!-- Text -->
  <text x="256" y="420" font-family="Arial, sans-serif" font-size="48" font-weight="bold" text-anchor="middle" fill="white">S</text>
</svg>'''

with open('icons/icon.svg', 'w') as f:
    f.write(svg_content)

print("Created SVG icon")
