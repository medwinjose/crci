# CRCI Live Dashboard

A standalone React + Vite dashboard to monitor the live mesh state of the CRCI network.

## Prerequisites
- Node.js 20 (LTS)
- Running CRCI backend on `localhost:8080`

## How to Run

1. Navigate to the dashboard directory:
   ```bash
   cd docs/dashboard
   ```
2. Install dependencies:
   ```bash
   npm install
   ```
3. Start the dev server:
   ```bash
   npm run dev
   ```
4. Open [http://localhost:5173](http://localhost:5173) in your browser.

## Production Build

To compile for production:
```bash
npm run build
```
The optimized bundle will be generated in `docs/dashboard/dist/`.

> **Note**: The Axum backend handles CORS dynamically, allowing local dashboard testing from any origin.
