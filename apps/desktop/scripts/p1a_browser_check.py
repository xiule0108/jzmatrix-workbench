"""P1-A offline UI smoke checks; the page is never given a real tool handle."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from urllib.parse import urlparse

from playwright.sync_api import Page, sync_playwright


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Check the P1-A offline novice flow")
    parser.add_argument("--url", default="http://127.0.0.1:1420")
    parser.add_argument("--artifacts", type=Path, default=None)
    return parser.parse_args()


def text(page: Page) -> str:
    return page.locator("body").inner_text()


def assert_contains(page: Page, phrase: str) -> None:
    assert phrase in text(page), f"missing visible copy: {phrase}"


def complete_demo_flow(page: Page) -> None:
    page.get_by_test_id("screen-connect").get_by_role("button", name="先看一个演示").click()
    page.get_by_test_id("screen-group").get_by_role("button", name="建立协作组（演示预览）").click()
    assert page.get_by_test_id("screen-progress").is_visible()


def focus_has_visible_ring(page: Page) -> bool:
    return bool(
        page.evaluate(
            """
            () => {
              const el = document.activeElement;
              if (!(el instanceof HTMLElement)) return false;
              const style = getComputedStyle(el);
              return style.outlineStyle !== 'none' && parseFloat(style.outlineWidth) > 0;
            }
            """
        )
    )


def run() -> dict[str, object]:
    args = parse_args()
    requests: list[dict[str, str]] = []
    console_errors: list[str] = []
    page_errors: list[str] = []
    screenshots: list[str] = []

    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(headless=True)
        page = browser.new_page(viewport={"width": 1280, "height": 800}, device_scale_factor=1)
        page.on(
            "request",
            lambda request: requests.append(
                {"url": request.url, "resource_type": request.resource_type}
            ),
        )
        page.on(
            "console",
            lambda message: console_errors.append(message.text)
            if message.type == "error"
            else None,
        )
        page.on("pageerror", lambda error: page_errors.append(str(error)))

        page.goto(args.url, wait_until="networkidle")
        assert page.title() == "介子九维协作工作台 · 演示"
        assert page.get_by_test_id("screen-connect").is_visible()
        assert_contains(page, "演示数据")
        assert_contains(page, "未检查")
        assert_contains(page, "只读能力待确认")
        assert_contains(page, "不会读取对话正文")
        assert page.locator("button[disabled]").filter(has_text="只读能力待确认").count() == 2

        if args.artifacts:
            args.artifacts.mkdir(parents=True, exist_ok=True)
            desktop_path = args.artifacts / "p1a-desktop-connect.png"
            page.screenshot(path=str(desktop_path), full_page=True)
            screenshots.append(str(desktop_path))

        complete_demo_flow(page)
        assert_contains(page, "协作组工作包")
        assert_contains(page, "尚未建立")
        assert_contains(page, "已写入本机消息记录")
        assert_contains(page, "尚未确认送达")
        assert_contains(page, "最近有动静")
        assert_contains(page, "暂时没有可核对的进度记录")
        assert page.locator("[data-state='local_written']").count() == 1
        assert page.locator("[data-state='sent_not_confirmed']").count() == 1

        page.locator("[data-evidence='delivery']").first.click()
        dialog = page.get_by_role("dialog")
        assert dialog.is_visible()
        assert "目前不能确认" in dialog.inner_text()
        page.keyboard.press("Escape")
        assert page.get_by_role("dialog").count() == 0

        page.keyboard.press("Tab")
        assert page.evaluate("document.activeElement?.tagName") in {"BUTTON", "TEXTAREA", "SUMMARY"}
        assert focus_has_visible_ring(page)

        page.set_viewport_size({"width": 390, "height": 844})
        page.goto(args.url, wait_until="networkidle")
        complete_demo_flow(page)
        assert page.evaluate("document.documentElement.scrollWidth <= window.innerWidth")
        if args.artifacts:
            mobile_path = args.artifacts / "p1a-mobile-progress.png"
            page.screenshot(path=str(mobile_path), full_page=True)
            screenshots.append(str(mobile_path))

        browser.close()

    allowed_hosts = {urlparse(args.url).hostname}
    non_loopback = [
        item
        for item in requests
        if urlparse(item["url"]).hostname not in allowed_hosts
        and urlparse(item["url"]).hostname not in {"localhost", "127.0.0.1"}
    ]
    result = {
        "ok": not console_errors and not page_errors and not non_loopback,
        "checks": 19,
        "console_errors": console_errors,
        "page_errors": page_errors,
        "request_count": len(requests),
        "non_loopback_requests": non_loopback,
        "screenshots": screenshots,
    }
    print(json.dumps(result, ensure_ascii=False, indent=2))
    assert result["ok"], result
    return result


if __name__ == "__main__":
    run()
