from pathlib import Path
from fastapi import FastAPI, Request
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles
from fastapi.responses import FileResponse

from .config import ALLOWED_ORIGINS, UPLOAD_DIR
from .database import engine, Base
from .routers import auth_router, users_router, posts_router

# Create database tables
Base.metadata.create_all(bind=engine)

# Frontend build directory
FRONTEND_DIR = Path(__file__).resolve().parent.parent.parent / "frontend" / "dist"

app = FastAPI(
    title="Thought Manifold API",
    description="지식 공유 커뮤니티 API - 에세이, 논문, 리포트 공유 플랫폼",
    version="1.0.0"
)

# CORS middleware
app.add_middleware(
    CORSMiddleware,
    allow_origins=ALLOWED_ORIGINS + ["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Mount static files for uploads
app.mount("/uploads", StaticFiles(directory=str(UPLOAD_DIR)), name="uploads")

# Include API routers
app.include_router(auth_router)
app.include_router(users_router)
app.include_router(posts_router)

@app.get("/api/health")
async def health_check():
    return {"status": "healthy"}

# Serve static assets from frontend build
if FRONTEND_DIR.exists():
    app.mount("/assets", StaticFiles(directory=str(FRONTEND_DIR / "assets")), name="assets")

# Catch-all route for SPA - must be last
@app.get("/{full_path:path}")
async def serve_spa(request: Request, full_path: str):
    """Serve the React SPA for all non-API routes"""
    # If it's an API route, skip (already handled by routers)
    if full_path.startswith("api/"):
        return {"detail": "Not Found"}
    
    # Try to serve static file first
    file_path = FRONTEND_DIR / full_path
    if file_path.is_file():
        return FileResponse(file_path)
    
    # Fallback to index.html for SPA routing
    index_path = FRONTEND_DIR / "index.html"
    if index_path.exists():
        return FileResponse(index_path)
    
    return {"message": "Welcome to Thought Manifold API", "docs": "/docs"}

