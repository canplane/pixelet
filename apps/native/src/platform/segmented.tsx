// @pxl/native/platform/segmented.tsx

/**
 * One of a few views of the same panel, on an iPhone: the system's segmented
 * control — SwiftUI's Picker in its segmented style — so the pick has the
 * press and the slide the system gives it. A segment holds its words alone,
 * as the system's does: UIKit's segment is a title or an image, never both.
 */

import { Host, Picker, Text } from "@expo/ui/swift-ui";
import { pickerStyle, tag } from "@expo/ui/swift-ui/modifiers";

export interface Segment<T extends string | number> {
  value: T;
  label: string;
}

export function Segmented<T extends string | number>({ segments, value, onChange }: { segments: readonly Segment<T>[]; value: T; onChange: (value: T) => void }) {
  return (
    <Host matchContents={{ vertical: true }}>
      <Picker selection={value} onSelectionChange={onChange} modifiers={[pickerStyle("segmented")]}>
        {segments.map((s) => <Text key={s.value} modifiers={[tag(s.value)]}>{s.label}</Text>)}
      </Picker>
    </Host>
  );
}
