import { Link } from 'react-router-dom';

const researchPrepSteps = [
    {
        title: '연구 질문 정의',
        detail:
            '한 문장으로 설명 가능한 핵심 질문을 먼저 고정하세요. “무엇을, 왜, 누구에게 유의미한가”가 명확해야 이후 설계와 논증이 흔들리지 않습니다.',
    },
    {
        title: '선행연구 매핑',
        detail:
            '핵심 키워드와 인용 네트워크를 기준으로 1차·2차 문헌을 구분해 정리하세요. 최신 논문(최근 3~5년)과 고전 논문을 균형 있게 포함합니다.',
    },
    {
        title: '방법론 설계',
        detail:
            '가설, 표본, 변수 정의, 분석 방법, 재현 절차를 사전에 문서화합니다. 정량 연구는 통계 검정력, 정성 연구는 코딩 기준을 명시하세요.',
    },
    {
        title: '윤리·데이터 관리',
        detail:
            'IRB/윤리 승인 필요 여부를 확인하고, 개인정보·민감 데이터 처리 기준을 기록합니다. 데이터 버전 관리와 원본 보존 정책도 함께 정합니다.',
    },
];

const imradSections = [
    {
        label: 'Introduction',
        points: '문제 배경, 연구 공백(gap), 기여(contribution)를 1-2-3 구조로 제시',
    },
    {
        label: 'Methods',
        points: '재현 가능성을 최우선으로 절차, 도구, 파라미터, 제외 기준을 상세화',
    },
    {
        label: 'Results',
        points: '해석보다 사실 중심 보고. 표/그림은 본문과 독립적으로 이해 가능하게 구성',
    },
    {
        label: 'Discussion',
        points: '결과 의미, 기존 연구 비교, 한계, 후속 연구 방향을 균형 있게 기술',
    },
];

const qualityChecklist = [
    '초록에 배경-방법-결과-결론이 모두 포함되었는가',
    '제목/키워드가 데이터베이스 검색 의도와 일치하는가',
    '그림·표 번호와 본문 참조가 모두 맞는가',
    '통계값(p, CI, effect size) 표기가 학회/저널 스타일을 따르는가',
    '참고문헌 포맷(APA, IEEE, Vancouver 등)이 100% 일관적인가',
    '표절/중복게재/자기표절 위험 문장을 사전 점검했는가',
    '저자 기여(CRediT), 이해상충, 데이터 가용성 문구를 준비했는가',
    '커버레터에 저널 적합성과 연구 기여를 명확히 작성했는가',
];

const aiReviewPolicy = [
    {
        title: 'AI 1차 심사(편집자 심사 대체)',
        detail:
            '논문 카테고리 투고 시 AI가 주제 적합성, 형식 준수, 기초 논리 일관성, 명백한 품질 결함을 자동 점검합니다.',
    },
    {
        title: 'AI 동료심사(피어리뷰 대체)',
        detail:
            'AI가 방법론 타당성, 결과 해석 정합성, 인용 적절성, 재현 가능성 관점에서 다각도 심사 코멘트를 생성합니다.',
    },
    {
        title: '수정 후 재심',
        detail:
            '저자는 AI 심사 코멘트별 수정 근거를 제출하고 재심사를 요청할 수 있습니다. 승인 시 제작·출간 단계로 이동합니다.',
    },
];

const generalPublicationTimeline = [
    {
        step: '1. 저널 선정',
        duration: '1-2주',
        description:
            'Aims & Scope, 최근 게재 논문, 투고 규정, 평균 심사 기간, APC(게재료)를 비교해 목표 저널을 결정합니다.',
    },
    {
        step: '2. 투고 패키지 준비',
        duration: '수일',
        description:
            '원고 파일, 커버레터, 그래픽 초록(요구 시), 보조자료, 윤리 문서, 저자 정보 메타데이터를 포맷에 맞게 맞춥니다.',
    },
    {
        step: '3. 편집자 1차 심사(Desk Review)',
        duration: '1-3주',
        description:
            '저널 범위 적합성, 윤리·형식 준수 여부를 확인합니다. 범위 밖이거나 형식 미달이면 초기 거절될 수 있습니다.',
    },
    {
        step: '4. 동료심사(Peer Review)',
        duration: '4-12주',
        description:
            '외부 심사위원이 방법론, 참신성, 해석 정합성, 인용 품질을 검토하여 Minor/Major Revision 또는 Reject 의견을 제시합니다.',
    },
    {
        step: '5. 저자 수정(R&R)',
        duration: '2-8주',
        description:
            '수정 원고와 Response Letter를 함께 제출합니다. 모든 코멘트에 대해 “수정 위치 + 근거”를 명시적으로 답변해야 합니다.',
    },
    {
        step: '6. 최종 판정',
        duration: '1-4주',
        description:
            '편집자가 심사 의견을 종합해 Accept/Reject를 결정합니다. 필요 시 추가 라운드 심사가 진행됩니다.',
    },
    {
        step: '7. 제작·출간',
        duration: '2-6주',
        description:
            '교정쇄(proof) 확인, DOI 부여, 온라인 선출간(Online First), 최종 호(issue) 배정 순으로 진행됩니다.',
    },
];

const communityPublicationTimeline = [
    {
        step: '1. 논문 업로드',
        duration: '즉시',
        description:
            '논문 카테고리로 원고를 업로드하고, 필요 시 인용 문헌 ID와 첨부파일을 함께 제출합니다.',
    },
    {
        step: '2. AI 1차 심사(편집자 심사 대체)',
        duration: '수분-24시간',
        description:
            '주제 적합성, 형식 준수, 기초 논리 일관성을 자동 점검하고, 기준 미달 시 보완 필요 상태를 반환합니다.',
    },
    {
        step: '3. AI 동료심사(피어리뷰 대체)',
        duration: '수시간-48시간',
        description:
            '방법론 타당성, 참신성, 해석 정합성, 인용 품질을 평가해 Accept/Minor/Major/Reject를 산출합니다.',
    },
    {
        step: '4. 저자 수정 및 재심사',
        duration: '작성자 대응 시간',
        description:
            '저자는 심사 코멘트에 따라 원고를 수정하고 재심사를 요청할 수 있습니다. 수정 이력은 심사센터에서 추적합니다.',
    },
    {
        step: '5. 최종 판정 반영',
        duration: '자동 반영',
        description:
            '최신 완료 심사 결과가 Accept인 경우에만 게시글로 공개됩니다. Accept 외 판정은 비공개 상태를 유지합니다.',
    },
    {
        step: '6. 커뮤니티 공개/탐색',
        duration: '즉시',
        description:
            '공개된 논문만 메인 탐색, 상세, 사용자 게시글 목록에 노출됩니다. 미공개 논문은 심사센터에서만 확인 가능합니다.',
    },
];

const rebuttalTips = [
    '감정적 표현 대신 증거(추가 실험, 통계, 문헌)로 답변하고, AI 코멘트 ID별로 대응 항목을 분리하세요.',
    '수정하지 않은 항목은 “왜 수정하지 않았는지”를 정중하게 설명하세요.',
    'Response Letter는 코멘트 번호를 원문 그대로 인용해 추적성을 높이세요.',
    '추가 분석으로 결론이 달라지면 본문 결론도 함께 업데이트하세요.',
];

function GuidelinesPage() {
    return (
        <main className="guidelines-page">
            <section className="guidelines-hero">
                <div className="container guidelines-hero-content">
                    <span className="hero-badge">
                        <span className="hero-badge-icon">📚</span>
                        Research Paper Guidelines
                    </span>
                    <h1>연구 논문 작성과 출간<br />전체 프로세스 가이드</h1>
                    <p>
                        아이디어 설계부터 저널 출간까지, 실제 연구자가 따라갈 수 있는
                        체크리스트 중심의 실무형 가이드입니다. 본 플랫폼에서는 편집자 1차 심사와 동료심사를 AI 심사로 대체합니다.
                    </p>
                </div>
            </section>

            <section className="about-section">
                <div className="container">
                    <h2 className="about-section-title">1. 연구 설계와 작성 준비</h2>
                    <div className="guidelines-grid">
                        {researchPrepSteps.map((item) => (
                            <article key={item.title} className="guidelines-card">
                                <h3>{item.title}</h3>
                                <p>{item.detail}</p>
                            </article>
                        ))}
                    </div>
                </div>
            </section>

            <section className="about-section guidelines-section-alt">
                <div className="container">
                    <h2 className="about-section-title">2. 본문 작성 구조(IMRaD)</h2>
                    <div className="guidelines-imrad">
                        {imradSections.map((section) => (
                            <article key={section.label} className="guidelines-imrad-item">
                                <h3>{section.label}</h3>
                                <p>{section.points}</p>
                            </article>
                        ))}
                    </div>
                </div>
            </section>

            <section className="about-section">
                <div className="container">
                    <h2 className="about-section-title">3. 투고 전 품질 점검 체크리스트</h2>
                    <div className="guidelines-checklist">
                        {qualityChecklist.map((item) => (
                            <p key={item} className="guidelines-check-item">
                                <span>✓</span>
                                {item}
                            </p>
                        ))}
                    </div>
                </div>
            </section>

            <section className="about-section guidelines-section-alt">
                <div className="container">
                    <h2 className="about-section-title">4. AI 심사 정책(본 플랫폼)</h2>
                    <div className="guidelines-grid">
                        {aiReviewPolicy.map((item) => (
                            <article key={item.title} className="guidelines-card">
                                <h3>{item.title}</h3>
                                <p>{item.detail}</p>
                            </article>
                        ))}
                    </div>
                </div>
            </section>

            <section className="about-section">
                <div className="container">
                    <h2 className="about-section-title">5. 논문 출간 프로세스 비교</h2>
                    <div className="guidelines-timeline-compare">
                        <article className="guidelines-timeline-panel">
                            <h3 className="guidelines-timeline-panel-title">일반적인 학술 저널 과정</h3>
                            <div className="guidelines-timeline">
                                {generalPublicationTimeline.map((item) => (
                                    <article key={`general-${item.step}`} className="guidelines-timeline-item">
                                        <div className="guidelines-timeline-head">
                                            <h3>{item.step}</h3>
                                            <span>{item.duration}</span>
                                        </div>
                                        <p>{item.description}</p>
                                    </article>
                                ))}
                            </div>
                        </article>
                        <article className="guidelines-timeline-panel">
                            <h3 className="guidelines-timeline-panel-title">해당 커뮤니티(Thought Manifold) 과정</h3>
                            <div className="guidelines-timeline">
                                {communityPublicationTimeline.map((item) => (
                                    <article key={`community-${item.step}`} className="guidelines-timeline-item">
                                        <div className="guidelines-timeline-head">
                                            <h3>{item.step}</h3>
                                            <span>{item.duration}</span>
                                        </div>
                                        <p>{item.description}</p>
                                    </article>
                                ))}
                            </div>
                        </article>
                    </div>
                </div>
            </section>

            <section className="about-section guidelines-section-alt">
                <div className="container">
                    <h2 className="about-section-title">6. AI 심사 대응(Response Letter) 핵심 원칙</h2>
                    <div className="guidelines-rebuttal">
                        {rebuttalTips.map((tip) => (
                            <p key={tip} className="guidelines-tip">
                                {tip}
                            </p>
                        ))}
                    </div>
                </div>
            </section>

            <section className="about-section about-cta-section">
                <div className="container about-cta-content">
                    <h2>가이드에 맞춰 지금 시작해보세요</h2>
                    <p>논문 카테고리에 원고를 업로드하고, 인용과 태그를 함께 관리해보세요.</p>
                    <div className="about-cta-actions">
                        <Link to="/upload" className="btn btn-primary btn-lg">
                            논문 업로드
                        </Link>
                        <Link to="/explore?category=paper" className="btn btn-secondary btn-lg">
                            논문 둘러보기
                        </Link>
                    </div>
                </div>
            </section>
        </main>
    );
}

export default GuidelinesPage;
