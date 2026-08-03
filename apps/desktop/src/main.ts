import { invoke } from "@tauri-apps/api/core";
import type { CliResponse, DoctorData, OfflineDemo } from "@jzmatrix/protocol";
import "./styles.css";

const healthStatus = document.querySelector<HTMLElement>("#health-status");
const healthDetails = document.querySelector<HTMLElement>("#health-details");
const demoStatus = document.querySelector<HTMLElement>("#demo-status");
const demoDetails = document.querySelector<HTMLElement>("#demo-details");

function setStatus(element: HTMLElement | null, status: string): void {
  if (element) element.textContent = status;
}

function setDetails(element: HTMLElement | null, details: string): void {
  if (element) element.textContent = details;
}

async function loadDesktopState(): Promise<void> {
  if (!("__TAURI_INTERNALS__" in window)) {
    setStatus(healthStatus, "桌面壳外");
    setDetails(healthDetails, "开发预览未连接 Tauri；请使用 npm run tauri:dev 运行桌面壳。");
    setStatus(demoStatus, "待桌面壳");
    setDetails(demoDetails, "离线 fixture 由 Rust core 提供，不在前端复制业务事实。");
    return;
  }

  try {
    const response = await invoke<CliResponse<DoctorData>>("doctor");
    setStatus(healthStatus, response.status);
    const summary = response.data.summary;
    setDetails(healthDetails, `必需检查 ${summary.pass} 项通过；未运行可选检查 ${summary.not_run} 项。`);
  } catch {
    setStatus(healthStatus, "blocked");
    setDetails(healthDetails, "共享 Rust doctor 未能返回可校验结果。");
  }

  try {
    const demo = await invoke<OfflineDemo>("offline_demo");
    const firstGroup = demo.groups[0];
    setStatus(demoStatus, "可用");
    setDetails(demoDetails, `${demo.title}：${firstGroup.goal}。本机写入状态 ${firstGroup.events[0].status}；外部发送 ${firstGroup.events[1].status}。`);
  } catch {
    setStatus(demoStatus, "blocked");
    setDetails(demoDetails, "随包离线 fixture 校验失败，未生成替代数据。");
  }
}

void loadDesktopState();
