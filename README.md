# thought-manifold
공부한 내용을 모아두려고 만든 사이트

## Docker로 실행

### 1) 사전 준비
- Docker Desktop 또는 Docker Engine + Docker Compose 설치

### 2) (선택) 환경변수 설정
- 루트 경로에 `.env` 파일을 만들고 필요한 값을 오버라이드할 수 있습니다.
- 기본값만으로도 로컬 실행은 가능합니다.

예시:

```env
SECRET_KEY=replace-with-strong-secret
GOOGLE_CLIENT_ID=
GOOGLE_CLIENT_SECRET=
GEMINI_API_KEY=
```

### 3) 컨테이너 실행

```bash
docker compose up -d --build
```

### 4) 접속
- 앱: `http://localhost:8000`
- 헬스체크: `http://localhost:8000/api/health`

### 5) 종료

```bash
docker compose down
```

DB 볼륨까지 삭제하려면:

```bash
docker compose down -v
```
