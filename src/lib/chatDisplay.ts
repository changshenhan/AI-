/**
 * 对话流展示：去掉模型常用的 markdown 星号（如 *斜体*、**粗体**）。
 * 仅用于「规划与对话」左侧流式区域；排期结果等系统文案勿用。
 */
export function stripChatAsterisks(text: string): string {
  return text.replace(/\*/g, "");
}
