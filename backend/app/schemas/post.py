from pydantic import BaseModel
from typing import Optional, List
from datetime import datetime
from enum import Enum

class PostCategory(str, Enum):
    ESSAY = "essay"
    PAPER = "paper"
    REPORT = "report"
    NOTE = "note"
    OTHER = "other"

class PostBase(BaseModel):
    title: str
    content: str
    summary: Optional[str] = None
    category: PostCategory = PostCategory.OTHER

class PostCreate(PostBase):
    pass

class PostUpdate(BaseModel):
    title: Optional[str] = None
    content: Optional[str] = None
    summary: Optional[str] = None
    category: Optional[PostCategory] = None

class AuthorInfo(BaseModel):
    id: int
    username: str
    display_name: Optional[str] = None
    avatar_url: Optional[str] = None
    
    class Config:
        from_attributes = True

class PostResponse(PostBase):
    id: int
    file_path: Optional[str] = None
    file_name: Optional[str] = None
    author_id: int
    author: AuthorInfo
    view_count: int
    like_count: int
    created_at: datetime
    updated_at: Optional[datetime] = None
    
    class Config:
        from_attributes = True

class PostListResponse(BaseModel):
    posts: List[PostResponse]
    total: int
    page: int
    per_page: int
