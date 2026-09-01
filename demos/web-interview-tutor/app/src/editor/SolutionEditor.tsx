import Editor, { loader } from "@monaco-editor/react";
import * as monaco from "monaco-editor";
import { useId } from "react";

// Use the locally bundled Monaco ESM build rather than the loader's CDN
// fallback. Learner code and editor assets stay within the static app.
loader.config({ monaco });

export interface SolutionEditorProps {
  readonly value: string;
  readonly entryFunctionName: string;
  readonly disabled?: boolean;
  readonly onChange: (value: string) => void;
  readonly onReset?: () => void;
}

/**
 * Controlled JavaScript editor for a single pure-function interview attempt.
 * Monaco only provides editing/language tooling; it never evaluates learner
 * source. Execution is delegated exclusively to the compiled sandbox workflow.
 */
export function SolutionEditor({
  value,
  entryFunctionName,
  disabled = false,
  onChange,
  onReset,
}: SolutionEditorProps) {
  const descriptionId = useId();

  return (
    <section className="solution-editor" aria-labelledby="solution-editor-title">
      <div className="solution-editor-heading">
        <div>
          <p className="eyebrow">JavaScript solution</p>
          <h2 id="solution-editor-title">Write <code>{entryFunctionName}</code></h2>
        </div>
        {onReset && (
          <button
            className="button button-secondary"
            type="button"
            onClick={onReset}
            disabled={disabled}
          >
            Reset starter
          </button>
        )}
      </div>
      <p id={descriptionId} className="editor-help">
        Write a pure JavaScript function with this exact name. It receives
        JSON-compatible arguments and must return a JSON-compatible value.
      </p>
      <div className="monaco-shell" aria-describedby={descriptionId}>
        <Editor
          height="min(54vh, 620px)"
          language="javascript"
          theme="vs-dark"
          value={value}
          onChange={(nextValue) => onChange(nextValue ?? "")}
          loading={<p className="editor-loading">Loading local editor…</p>}
          options={{
            automaticLayout: true,
            // Monaco 0.56 defaults to Chromium's experimental EditContext
            // API (`<div role="textbox" class="native-edit-context">`).
            // Browser Vim extensions (Vimium, cVim, Tridactyl) intercept
            // keystrokes from that surface but recognise a real <textarea>,
            // so we opt back into the legacy textarea input path.
            // See microsoft/monaco-editor#5168.
            editContext: false,
            minimap: { enabled: false },
            fontSize: 14,
            lineNumbersMinChars: 3,
            tabSize: 2,
            insertSpaces: true,
            wordWrap: "on",
            scrollBeyondLastLine: false,
            padding: { top: 14, bottom: 14 },
            readOnly: disabled,
            ariaLabel: `JavaScript editor for ${entryFunctionName}`,
          }}
        />
      </div>
    </section>
  );
}
