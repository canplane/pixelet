/** 위치를 찾는 중일 때 보여줄 스피너 */
export function LoadingSpinner() {
  return (
    <div
      style={{
        position: 'absolute',
        top: '50%',
        left: '50%',
        transform: 'translate(-50%, -50%)',
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        gap: '12px',
        zIndex: 1500,
        pointerEvents: 'none',
      }}
    >
      {/* 실제 스피너 도형 */}
      <div className="spinner" />

      {/* 텍스트 안내 */}
      <div
        style={{
          color: 'white',
          fontSize: '14px',
          fontWeight: '600',
          textShadow: '0 2px 4px rgba(0,0,0,0.5)',
          letterSpacing: '-0.02em',
        }}
      >
        위치 확인 중...
      </div>

      <style>{`
          .spinner {
            width: 32px;
            height: 32px;
            border: 3px solid rgba(255, 255, 255, 0.2);
            border-top: 3px solid #007AFF; /* iOS/System Blue 포인트 컬러 */
            border-radius: 50%;
            animation: spin 0.8s cubic-bezier(0.4, 0, 0.2, 1) infinite;
          }
  
          @keyframes spin {
            0% { transform: rotate(0deg); }
            100% { transform: rotate(360deg); }
          }
        `}</style>
    </div>
  );
}
