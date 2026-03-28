import { useState, useRef, useEffect } from "react";
import { sendChatMessage, clearConversation } from "../commands/chat";

interface ToolCallInfo {
  id: string;
  toolName: string;
  success: boolean | null;
  message: string | null;
}

interface Message {
  id: string;
  role: "user" | "assistant";
  content: string;
  isStreaming: boolean;
  toolCalls: ToolCallInfo[];
}

let messageCounter = 0;
function nextId(): string {
  return `msg-${++messageCounter}`;
}

function ToolCallPill({ tc }: { tc: ToolCallInfo }) {
  const isPending = tc.success === null;
  const isSuccess = tc.success === true;

  const className = isPending
    ? "tool-pill tool-pill-pending"
    : isSuccess
      ? "tool-pill tool-pill-success"
      : "tool-pill tool-pill-error";

  const icon = isPending ? "\u2026" : isSuccess ? "\u2713" : "\u2717";
  const label = isPending
    ? tc.toolName
    : `${tc.toolName.replace("_", ".")} executed`;

  return (
    <span className={className}>
      <span className="tool-pill-icon">{icon}</span>
      <span className="tool-pill-label">{label}</span>
    </span>
  );
}

export default function ChatView() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [isStreaming, setIsStreaming] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  useEffect(() => {
    if (!isStreaming) {
      inputRef.current?.focus();
    }
  }, [isStreaming]);

  async function handleSend() {
    const text = input.trim();
    if (!text || isStreaming) return;

    setInput("");
    setIsStreaming(true);

    const userMsg: Message = {
      id: nextId(),
      role: "user",
      content: text,
      isStreaming: false,
      toolCalls: [],
    };
    const assistantId = nextId();
    const assistantMsg: Message = {
      id: assistantId,
      role: "assistant",
      content: "",
      isStreaming: true,
      toolCalls: [],
    };

    setMessages((prev) => [...prev, userMsg, assistantMsg]);

    try {
      await sendChatMessage(text, (event) => {
        if (event.event === "token") {
          setMessages((prev) =>
            prev.map((m) =>
              m.id === assistantId
                ? { ...m, content: m.content + event.data }
                : m,
            ),
          );
        } else if (event.event === "toolCallStart") {
          const tcId = nextId();
          setMessages((prev) =>
            prev.map((m) =>
              m.id === assistantId
                ? {
                    ...m,
                    toolCalls: [
                      ...m.toolCalls,
                      {
                        id: tcId,
                        toolName: event.data.toolName,
                        success: null,
                        message: null,
                      },
                    ],
                  }
                : m,
            ),
          );
        } else if (event.event === "toolCallResult") {
          setMessages((prev) =>
            prev.map((m) => {
              if (m.id !== assistantId) return m;
              const updatedCalls = [...m.toolCalls];
              for (let i = updatedCalls.length - 1; i >= 0; i--) {
                if (
                  updatedCalls[i].toolName === event.data.toolName &&
                  updatedCalls[i].success === null
                ) {
                  updatedCalls[i] = {
                    ...updatedCalls[i],
                    success: event.data.success,
                    message: event.data.message,
                  };
                  break;
                }
              }
              return { ...m, toolCalls: updatedCalls };
            }),
          );
        } else if (event.event === "done") {
          setMessages((prev) =>
            prev.map((m) =>
              m.id === assistantId ? { ...m, isStreaming: false } : m,
            ),
          );
          setIsStreaming(false);
        } else if (event.event === "error") {
          setMessages((prev) =>
            prev.map((m) =>
              m.id === assistantId
                ? { ...m, content: `Error: ${event.data}`, isStreaming: false }
                : m,
            ),
          );
          setIsStreaming(false);
        }
      });
    } catch (e) {
      setMessages((prev) =>
        prev.map((m) =>
          m.id === assistantId
            ? { ...m, content: `Error: ${e}`, isStreaming: false }
            : m,
        ),
      );
      setIsStreaming(false);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  }

  async function handleClear() {
    await clearConversation();
    setMessages([]);
  }

  return (
    <div className="chat-view">
      {messages.length > 0 && (
        <div className="chat-header">
          <button className="chat-clear-btn" onClick={handleClear}>
            New Conversation
          </button>
        </div>
      )}

      <div className="chat-messages">
        {messages.length === 0 && (
          <div className="chat-empty">
            <p className="chat-empty-title">Sierra</p>
            <p className="chat-empty-subtitle">
              Ask me to control your smart home devices
            </p>
          </div>
        )}
        {messages.map((msg) => (
          <div key={msg.id} className={`chat-bubble chat-bubble-${msg.role}`}>
            <div className="chat-bubble-content">
              {msg.isStreaming && msg.content === "" && msg.toolCalls.length === 0 ? (
                <span className="chat-thinking">
                  Thinking<span className="chat-thinking-dots" />
                </span>
              ) : (
                <>
                  {msg.content}
                  {msg.isStreaming && <span className="chat-cursor" />}
                </>
              )}
            </div>
            {msg.toolCalls.length > 0 && (
              <div className="chat-tool-calls">
                {msg.toolCalls.map((tc) => (
                  <ToolCallPill key={tc.id} tc={tc} />
                ))}
              </div>
            )}
          </div>
        ))}
        <div ref={messagesEndRef} />
      </div>

      <div className="chat-input-area">
        <div
          className={`chat-capsule ${isStreaming ? "chat-capsule-streaming" : ""}`}
        >
          <textarea
            ref={inputRef}
            className="chat-input"
            placeholder="Tell me what to do..."
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            disabled={isStreaming}
            rows={1}
          />
          <button
            className="chat-send-btn"
            onClick={handleSend}
            disabled={isStreaming || !input.trim()}
            aria-label="Send message"
          >
            <svg
              width="16"
              height="16"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
            >
              <line x1="12" y1="19" x2="12" y2="5" />
              <polyline points="5 12 12 5 19 12" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  );
}
