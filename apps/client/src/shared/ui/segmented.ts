// @pxl/client/shared/ui/segmented.ts

export interface SegmentedProps<T extends string | number> {
  segments: readonly { value: T; label: string }[];
  value: T;
  onChange: (value: T) => void;
}
