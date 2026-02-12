import { BrowserRouter as Router, Routes, Route } from 'react-router-dom';
import { AuthProvider } from './context/AuthContext';
import { Header, Footer } from './components';
import { Home, Upload, Login, PostDetail, EditPost, Profile, Admin, ReviewCenter, StaticInfoPage, AboutPage, GuidelinesPage } from './pages';
import './index.css';

function App() {
  return (
    <AuthProvider>
      <Router>
        <Header />
        <Routes>
          <Route path="/" element={<Home />} />
          <Route path="/explore" element={<Home />} />
          <Route path="/upload" element={<Upload />} />
          <Route path="/login" element={<Login />} />
          <Route path="/posts/:id" element={<PostDetail />} />
          <Route path="/posts/:id/edit" element={<EditPost />} />
          <Route path="/profile" element={<Profile />} />
          <Route path="/profile/:id" element={<Profile />} />
          <Route path="/admin" element={<Admin />} />
          <Route path="/reviews" element={<ReviewCenter />} />
          <Route path="/about" element={<AboutPage />} />
          <Route path="/guidelines" element={<GuidelinesPage />} />
          <Route
            path="/faq"
            element={<StaticInfoPage title="자주 묻는 질문" description="자주 발생하는 질문을 빠르게 확인할 수 있습니다." />}
          />
          <Route
            path="/contact"
            element={<StaticInfoPage title="문의하기" description="서비스 문의 및 제안 접수 채널을 준비 중입니다." />}
          />
          <Route
            path="/terms"
            element={<StaticInfoPage title="이용약관" description="서비스 이용약관을 정리하는 페이지입니다." />}
          />
          <Route
            path="/privacy"
            element={<StaticInfoPage title="개인정보처리방침" description="개인정보 처리 정책을 안내하는 페이지입니다." />}
          />
          <Route
            path="/copyright"
            element={<StaticInfoPage title="저작권 정책" description="저작권 정책과 신고 절차를 안내합니다." />}
          />
        </Routes>
        <Footer />
      </Router>
    </AuthProvider>
  );
}

export default App;
