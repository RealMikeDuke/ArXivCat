import { EFFORT_BUTTON_CLASS } from "../store";
import SegmentedControl from "./SegmentedControl";
import Select from "./Select";

interface ChatControlsProps {
  model: string;
  effort: string;
  onModelChange: (m: string) => void;
  onEffortChange: (e: string) => void;
}

export default function ChatControls({ model, effort, onModelChange, onEffortChange }: ChatControlsProps) {
  return (
    <>
      <SegmentedControl
        options={[{ value: "Flash", label: "Flash" }, { value: "Pro", label: "Pro" }]}
        value={model}
        onChange={onModelChange}
      />
      <Select
        options={[
          { value: "off", label: "No thinking" },
          { value: "low", label: "Low" },
          { value: "medium", label: "Medium" },
          { value: "high", label: "High" },
          { value: "max", label: "Max" },
        ]}
        value={effort}
        onChange={onEffortChange}
        buttonClassName={EFFORT_BUTTON_CLASS[effort]}
        chevron={false}
      />
    </>
  );
}
