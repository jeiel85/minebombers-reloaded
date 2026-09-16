export class Reconnector {
  private static readonly STORAGE_PREFIX = 'minebombers.resume.';

  public static getResumeToken(roomCode: string): string | null {
    try {
      return sessionStorage.getItem(this.STORAGE_PREFIX + roomCode);
    } catch {
      return null;
    }
  }

  public static saveResumeToken(roomCode: string, token: string): void {
    try {
      sessionStorage.setItem(this.STORAGE_PREFIX + roomCode, token);
    } catch { /* storage disabled */ }
  }

  public static clearResumeToken(roomCode: string): void {
    try {
      sessionStorage.removeItem(this.STORAGE_PREFIX + roomCode);
    } catch { /* storage disabled */ }
  }

  private retryCount = 0;
  private timer: ReturnType<typeof setTimeout> | null = null;

  public scheduleReconnect(callback: () => void): void {
    this.cancel();
    // Exponential backoff: 500ms, 1000ms, 2000ms, 4000ms capped at 5000ms
    const delay = Math.min(5000, 500 * (2 ** this.retryCount));
    this.retryCount++;
    this.timer = setTimeout(() => {
      this.timer = null;
      callback();
    }, delay);
  }

  public reset(): void {
    this.retryCount = 0;
    this.cancel();
  }

  public cancel(): void {
    if (this.timer) {
      clearTimeout(this.timer);
      this.timer = null;
    }
  }
}
