/** 나침반 */
export function Compass({ rotation }: { rotation: number }) {
  return (
    <div
      style={{
        position: 'absolute',
        top: '30px',
        right: '30px',
        width: '50px',
        height: '50px',
        zIndex: 100,
        background: 'rgba(0,0,0,0.5)',
        borderRadius: '50%',
        display: 'flex',
        justifyContent: 'center',
        alignItems: 'center',
        border: '1px solid white',
      }}
    >
      <div
        style={{
          width: '4px',
          height: '30px',
          background: 'linear-gradient(red 50%, white 50%)',
          transform: `rotate(${rotation}deg)`, // 전달받은 회전값 적용
          transition: 'transform 0.05s linear', // 부드러운 회전 효과
        }}
      />
      <span
        style={{
          position: 'absolute',
          top: '-20px',
          color: 'red',
          fontWeight: 'bold',
        }}
      >
        N
      </span>
    </div>
  );
}
