import { useMapStore } from '../../model/map-store';

/** 나침반 */
export function Compass() {
  const { bearing } = useMapStore((state) => state.viewState);
  const triggerOrientationReset = useMapStore(
    (state) => state.triggerOrientationReset,
  );

  return (
    <div
      onClick={triggerOrientationReset}
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
        cursor: 'pointer', // 클릭 가능함 표시
        transition: 'background 0.2s',
      }}
      onMouseEnter={(e) =>
        (e.currentTarget.style.background = 'rgba(0,0,0,0.7)')
      }
      onMouseLeave={(e) =>
        (e.currentTarget.style.background = 'rgba(0,0,0,0.5)')
      }
    >
      <div
        style={{
          width: '4px',
          height: '30px',
          background: 'linear-gradient(red 50%, white 50%)',
          transform: `rotate(${bearing}deg)`, // 전달받은 회전값 적용
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
