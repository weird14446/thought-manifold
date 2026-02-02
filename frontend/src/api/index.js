import axios from 'axios';

const API_BASE_URL = 'http://localhost:8000/api';

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
        const formData = new FormData();
        formData.append('username', username);
        formData.append('password', password);
        const response = await api.post('/auth/login', formData, {
            headers: { 'Content-Type': 'multipart/form-data' },
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
};

export default api;
