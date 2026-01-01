/**
 * [triggerDownload]
 * 생성된 Blob 또는 Uint8Array 데이터를 브라우저의 다운로드 트리거를 통해 로컬 파일로 저장합니다.
 */
export function triggerDownload(
  data: Blob | Uint8Array,
  filename: string,
  mimeType: string = 'application/octet-stream', // 기본값 설정
) {
  if (typeof window === 'undefined') return;

  let blob: Blob;

  if (data instanceof Blob) {
    // 1. 이미 Blob인 경우 (이미지 등): 이미 내부에 type(image/png 등)이 정의되어 있음
    blob = data;
  } else {
    // 2. Uint8Array인 경우 (바이너리): 표준 바이너리 타입 적용
    /**
     * SharedArrayBuffer 대응을 위해 복사본 생성
     * 일부 Uint8Array는 SharedArrayBuffer를 buffer로 가질 수 있으며, 이는 BlobPart와 호환되지 않습니다.
     * 새로운 Uint8Array를 생성하여 데이터를 복사(Copy)함으로써 표준 ArrayBuffer로 변환합니다.
     */
    const safeData = new Uint8Array(data);
    // 🎯 고정된 문자열 대신 매개변수로 받은 mimeType을 사용합니다.
    blob = new Blob([safeData], { type: mimeType });
  }

  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;

  // DOM에 잠시 추가했다가 클릭 후 제거 (일부 브라우저 호환성 방어)
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);

  URL.revokeObjectURL(url);
}
