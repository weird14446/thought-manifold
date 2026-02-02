import os
import uuid
from typing import Optional
from fastapi import APIRouter, Depends, HTTPException, status, UploadFile, File, Form
from sqlalchemy.orm import Session

from ..database import get_db
from ..models import Post, User
from ..schemas import PostCreate, PostResponse, PostListResponse, PostCategory
from ..config import UPLOAD_DIR
from .auth import get_current_user

router = APIRouter(prefix="/api/posts", tags=["posts"])

@router.get("/", response_model=PostListResponse)
async def get_posts(
    page: int = 1,
    per_page: int = 10,
    category: Optional[str] = None,
    search: Optional[str] = None,
    db: Session = Depends(get_db)
):
    query = db.query(Post)
    
    if category:
        query = query.filter(Post.category == category)
    if search:
        query = query.filter(Post.title.contains(search) | Post.content.contains(search))
    
    total = query.count()
    posts = query.order_by(Post.created_at.desc()).offset((page - 1) * per_page).limit(per_page).all()
    
    return PostListResponse(
        posts=posts,
        total=total,
        page=page,
        per_page=per_page
    )

@router.get("/{post_id}", response_model=PostResponse)
async def get_post(post_id: int, db: Session = Depends(get_db)):
    post = db.query(Post).filter(Post.id == post_id).first()
    if not post:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Post not found"
        )
    # Increment view count
    post.view_count += 1
    db.commit()
    return post

@router.post("/", response_model=PostResponse)
async def create_post(
    title: str = Form(...),
    content: str = Form(...),
    summary: Optional[str] = Form(None),
    category: str = Form("other"),
    file: Optional[UploadFile] = File(None),
    db: Session = Depends(get_db),
    current_user: User = Depends(get_current_user)
):
    file_path = None
    file_name = None
    
    if file:
        # Generate unique filename
        file_ext = os.path.splitext(file.filename)[1]
        unique_name = f"{uuid.uuid4()}{file_ext}"
        file_path = str(UPLOAD_DIR / unique_name)
        file_name = file.filename
        
        # Save file
        with open(file_path, "wb") as buffer:
            content_bytes = await file.read()
            buffer.write(content_bytes)
    
    db_post = Post(
        title=title,
        content=content,
        summary=summary,
        category=category,
        file_path=file_path,
        file_name=file_name,
        author_id=current_user.id
    )
    db.add(db_post)
    db.commit()
    db.refresh(db_post)
    return db_post

@router.delete("/{post_id}")
async def delete_post(
    post_id: int,
    db: Session = Depends(get_db),
    current_user: User = Depends(get_current_user)
):
    post = db.query(Post).filter(Post.id == post_id).first()
    if not post:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Post not found"
        )
    if post.author_id != current_user.id:
        raise HTTPException(
            status_code=status.HTTP_403_FORBIDDEN,
            detail="Not authorized to delete this post"
        )
    
    # Delete file if exists
    if post.file_path and os.path.exists(post.file_path):
        os.remove(post.file_path)
    
    db.delete(post)
    db.commit()
    return {"message": "Post deleted successfully"}

@router.post("/{post_id}/like")
async def like_post(
    post_id: int,
    db: Session = Depends(get_db),
    current_user: User = Depends(get_current_user)
):
    post = db.query(Post).filter(Post.id == post_id).first()
    if not post:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail="Post not found"
        )
    post.like_count += 1
    db.commit()
    return {"message": "Post liked", "like_count": post.like_count}
