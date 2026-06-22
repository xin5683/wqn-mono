interface RichTextEditorCommandChain {
  focus: () => RichTextEditorCommandChain;
  setResizableImage: (attrs: {
    src: string;
    alt?: string;
  }) => RichTextEditorCommandChain;
  run: () => boolean;
}

interface RichTextEditorInstance {
  chain: () => RichTextEditorCommandChain;
  commands: {
    setContent: (html: string) => void;
  };
}

export interface RichTextEditorHandle {
  editor: RichTextEditorInstance | null;
  setContent: (html: string) => void;
}

export interface RichTextEditorProps {
  initialContent?: string;
  onChange?: (content: string) => void;
  placeholder?: string;
  className?: string;
  disabled?: boolean;
  height?: string;
  minHeight?: string;
  maxHeight?: string;
  maxLength?: number;
  showCharacterCount?: boolean;
}
