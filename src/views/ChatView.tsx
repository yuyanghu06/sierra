import { useState, useRef, useEffect } from "react";
import { sendChatMessage, clearConversation } from "../commands/chat";

interface Message {
  id: string;
  role: "user" | "assistant";
  content: string;
  isStreaming: boolean;
}

let messageCounter = 0;
function nextId(): string {
  return `msg-${++messageCounter}`;
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
    };
    const assistantId = nextId();
    const assistantMsg: Message = {
      id: assistantId,
      role: "assistant",
      content: "",
      isStreaming: true,
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
              {msg.isStreaming && msg.content === "" ? (
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
