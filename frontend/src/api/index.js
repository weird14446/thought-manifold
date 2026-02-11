import axios from 'axios';

const API_BASE_URL = '/api';

const api = axios.create({
    baseURL: API_BASE_URL,
    headers: {
        'Content-Type': 'application/json',
    },
});

// Request interceptor to add auth token
api.interceptors.request.use((config) => {
    const token = localStorage.getItem('token');
    if (token) {
        config.headers.Authorization = `Bearer ${token}`;
    }
    return config;
});

// Auth API
export const authAPI = {
    login: async (username, password) => {
        const params = new URLSearchParams();
        params.append('username', username);
        params.append('password', password);
        const response = await api.post('/auth/login', params, {
            headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
        });
        return response.data;
    },
    register: async (userData) => {
        const response = await api.post('/auth/register', userData);
        return response.data;
    },
    getMe: async () => {
        const response = await api.get('/auth/me');
        return response.data;
    },
};

// Posts API
export const postsAPI = {
    getPosts: async (page = 1, perPage = 10, category = null, search = null) => {
        const params = { page, per_page: perPage };
        if (category) params.category = category;
        if (search) params.search = search;
        const response = await api.get('/posts', { params });
        return response.data;
    },
    getPost: async (id) => {
        const response = await api.get(`/posts/${id}`);
        return response.data;
    },
    createPost: async (postData) => {
        const formData = new FormData();
        formData.append('title', postData.title);
        formData.append('content', postData.content);
        if (postData.summary) formData.append('summary', postData.summary);
        formData.append('category', postData.category || 'other');
        if (postData.file) formData.append('file', postData.file);

        const response = await api.post('/posts', formData, {
            headers: { 'Content-Type': 'multipart/form-data' },
        });
        return response.data;
    },
    deletePost: async (id) => {
        const response = await api.delete(`/posts/${id}`);
        return response.data;
    },
    likePost: async (id) => {
        const response = await api.post(`/posts/${id}/like`);
        return response.data;
    },
    updatePost: async (id, postData) => {
        const formData = new FormData();
        formData.append('title', postData.title);
        formData.append('content', postData.content);
        if (postData.summary !== undefined) formData.append('summary', postData.summary || '');
        formData.append('category', postData.category || 'other');
        if (postData.removeFile) formData.append('remove_file', 'true');
        if (postData.file) formData.append('file', postData.file);

        const response = await api.put(`/posts/${id}`, formData, {
            headers: { 'Content-Type': 'multipart/form-data' },
        });
        return response.data;
    },
};

// Users API
export const usersAPI = {
    getUser: async (id) => {
        const response = await api.get(`/users/${id}`);
        return response.data;
    },
    getUsers: async (skip = 0, limit = 20) => {
        const response = await api.get('/users', { params: { skip, limit } });
        return response.data;
    },
    updateProfile: async (data) => {
        const response = await api.put('/users/me', data);
        return response.data;
    },
    getUserPosts: async (userId) => {
        const response = await api.get(`/users/${userId}/posts`);
        return response.data;
    },
};

// Comments API
export const commentsAPI = {
    list: async (postId) => {
        const response = await api.get(`/posts/${postId}/comments`);
        return response.data;
    },
    create: async (postId, content) => {
        const response = await api.post(`/posts/${postId}/comments`, { content });
        return response.data;
    },
    delete: async (postId, commentId) => {
        const response = await api.delete(`/posts/${postId}/comments/${commentId}`);
        return response.data;
    },
};

// Admin API
export const adminAPI = {
    getStats: async () => {
        const response = await api.get('/admin/stats');
        return response.data;
    },
    getUsers: async () => {
        const response = await api.get('/admin/users');
        return response.data;
    },
    updateUserRole: async (userId, isAdmin) => {
        const response = await api.put(`/admin/users/${userId}/role`, { is_admin: isAdmin });
        return response.data;
    },
    deleteUser: async (userId) => {
        const response = await api.delete(`/admin/users/${userId}`);
        return response.data;
    },
    deletePost: async (postId) => {
        const response = await api.delete(`/admin/posts/${postId}`);
        return response.data;
    },
    deleteComment: async (commentId) => {
        const response = await api.delete(`/admin/comments/${commentId}`);
        return response.data;
    },
};

export default api;
