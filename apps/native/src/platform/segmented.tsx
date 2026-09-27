// @pxl/native/platform/segmented.tsx

/**
 * One of a few views of the same panel, on a device: the system's segmented
 * control — SwiftUI's Picker in its segmented style, Compose's single-choice
 * row — so the pick has the press and the slide the system gives it. A
 * segment holds its words alone, as the system's does: UIKit's segment is a
 * title or an image, never both.
 */

import { Platform } from "react-native";
import { Host as SwiftHost, Picker, Text as SwiftText } from "@expo/ui/swift-ui";
import { pickerStyle, tag } from "@expo/ui/swift-ui/modifiers";
import { Host as ComposeHost, SegmentedButton, SingleChoiceSegmentedButtonRow, Text as ComposeText } from "@expo/ui/jetpack-compose";
import { fillMaxWidth } from "@expo/ui/jetpack-compose/modifiers";

export interface Segment<T extends string | number> {
  value: T;
  label: string;
}

export function Segmented<T extends string | number>({ segments, value, onChange }: { segments: readonly Segment<T>[]; value: T; onChange: (value: T) => void }) {
  if (Platform.OS === "ios") {
    return (
      <SwiftHost matchContents={{ vertical: true }}>
        <Picker selection={value} onSelectionChange={onChange} modifiers={[pickerStyle("segmented")]}>
          {segments.map((s) => <SwiftText key={s.value} modifiers={[tag(s.value)]}>{s.label}</SwiftText>)}
        </Picker>
      </SwiftHost>
    );
  }
  return (
    <ComposeHost matchContents={{ vertical: true }}>
      <SingleChoiceSegmentedButtonRow modifiers={[fillMaxWidth()]}>
        {segments.map((s) => (
          <SegmentedButton key={s.value} selected={s.value === value} onClick={() => onChange(s.value)}>
            <SegmentedButton.Label>
              <ComposeText>{s.label}</ComposeText>
            </SegmentedButton.Label>
          </SegmentedButton>
        ))}
      </SingleChoiceSegmentedButtonRow>
    </ComposeHost>
  );
}
