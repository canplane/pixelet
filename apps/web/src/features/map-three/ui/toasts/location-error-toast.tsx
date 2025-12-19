/** 위치 오류 알림 토스트 */
export function LocationErrorToast({ message }: { message: string }) {
  return (
    <div
      style={{
        position: 'absolute',
        top: '50%',
        left: '50%',
        transform: 'translate(-50%, -50%)',
        backgroundColor: 'rgba(0, 0, 0, 0.8)',
        color: 'white',
        padding: '12px 20px',
        borderRadius: '12px',
        zIndex: 2000, // 최상단
        textAlign: 'center',
        pointerEvents: 'none', // 클릭 방해 금지
        boxShadow: '0 4px 20px rgba(0,0,0,0.5)',
        border: '1px solid rgba(255,255,255,0.2)',
        display: 'flex',
        flexDirection: 'column',
        gap: '4px',
        maxWidth: '80%',
        animation: 'fadeIn 0.3s ease-out',
      }}
    >
      <div style={{ fontWeight: 'bold', fontSize: '15px' }}>📍 {message}</div>
      <div style={{ fontSize: '12px', opacity: 0.8 }}>
        실외로 이동하거나 잠시만 기다려주세요.
      </div>

      <style>{`
          @keyframes fadeIn {
            from { opacity: 0; transform: translate(-50%, -40%); }
            to { opacity: 1; transform: translate(-50%, -50%); }
          }
        `}</style>
    </div>
  );
}
