# Signal TEE Web Frontend Design

## Overview

A trust-focused web frontend for the Signal TEE bot that helps users:
1. Discover bots and their usernames
2. Verify TEE attestation
3. Confirm Signal identity keys
4. Message bots via Signal deep links

## Tech Stack

- **Framework**: React + Vite
- **Hosting**: Vercel
- **Styling**: Tailwind CSS + Framer Motion
- **Design**: Apple/iOS glassmorphism

## Page Structure

### 1. Hero / Landing
- Animated intro with product name
- Tagline: "Private AI conversations secured by hardware"
- Floating glass card showing trust promise

### 2. Bot Directory
- Grid of glass cards (one per registered bot)
- Each card shows:
  - Bot username
  - Signal deep link button
  - "Verify Security" button

### 3. Verification Panel
- Simple trust indicators for end users
- Collapsible technical details for auditors

## Visual Design

### Colors
- Background: Gradient `#f5f7fa` → `#e4e8f0` with animated mesh blobs
- Glass cards: `rgba(255,255,255,0.7)` + `backdrop-filter: blur(20px)`
- Accent: `linear-gradient(135deg, #667eea, #764ba2)`
- Text: `#1a1a2e`

### Liquid Glass Buttons
- Semi-transparent gradient fill
- Soft inner glow + outer shadow
- Hover: scale(1.02), increased blur
- Click: scale(0.98) press animation

### Animations
- Hero: Staggered fade-in + slide-up
- Background: Slow floating/morphing blobs
- Cards: Viewport fade-in
- Checkmarks: Pop animation on verify

## API Integration

### New Endpoints (Registration Proxy)

```
GET /v1/bots
Response: [{
  username: string,
  signal_link: string,
  identity_key: string,
  registered_at: string
}]

GET /v1/attestation
Response: {
  in_tee: boolean,
  app_id: string,
  compose_hash: string,
  tdx_quote_base64: string,
  verification_url: string
}
```

### CORS
- Allow origin from Vercel domain

## Verification Flow

### End Users
- ✅ "Running in secure hardware (Intel TDX)"
- ✅ "Code verified by Phala Network"
- ✅ "Messages encrypted end-to-end"

### Technical Auditors
- Signal identity key (hex + copy)
- App ID (links to Phala dashboard)
- Compose hash (links to GitHub)
- Full TDX quote (base64, copyable)
- Interactive challenge verification

## Project Structure

```
web/
├── src/
│   ├── components/
│   ├── hooks/
│   ├── styles/
│   ├── lib/
│   ├── App.tsx
│   └── main.tsx
├── public/
├── package.json
├── vite.config.ts
├── tailwind.config.js
└── vercel.json
```
