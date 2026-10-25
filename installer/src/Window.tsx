import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { LogicalSize } from "@tauri-apps/api/dpi";

const currentWindow = getCurrentWebviewWindow();

export const Window = {
    async setFullScreen(enable: boolean) {
        await currentWindow.setFullscreen(enable);
    },
    async setSize(width: number, height: number) {
        await currentWindow.setSize(new LogicalSize(width, height));
    },
    async center() {
        await currentWindow.center();
    },
    async startDragging() {
        await currentWindow.startDragging();
    },
    async hide() {
        await currentWindow.hide();
    },
    async close() {
        await currentWindow.close();
    }
};