import { invoke, Channel } from "@tauri-apps/api/core";

export type ChatEvent =
  | { event: "token"; data: string }
  | { event: "toolCallStart"; data: { toolName: string } }
  | { event: "toolCallResult"; data: { toolName: string; success: boolean; message: string } }
  | { event: "done"; data: { fullResponse: string } }
  | { event: "error"; data: string };

export async function sendChatMessage(
  message: string,
  onEvent: (event: ChatEvent) => void,
): Promise<void> {
  const channel = new Channel<ChatEvent>();
  channel.onmessage = onEvent;
  await invoke("send_chat_message", { message, onEvent: channel });
}

export async function clearConversation(): Promise<void> {
  return invoke("clear_conversation");
}

export async function checkOllamaHealth(): Promise<boolean> {
  return invoke<boolean>("check_ollama_health");
}

export async function listModels(): Promise<string[]> {
  return invoke<string[]>("list_models");
}
