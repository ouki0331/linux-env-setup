#!/usr/bin/env python3
"""
交互式 CLI HTTP 调试/请求工具 (Interactive CLI HTTP Client)
零第三方依赖，纯 Python 3 原生库实现。
支持向导式交互、默认值快速回车、响应 JSON 高亮格式化、cURL 命令导出。
"""

import sys
import os
import json
import time
import ssl
import urllib.request
import urllib.error
import urllib.parse
from pathlib import Path

# ==========================================
# 终端颜色与样式
# ==========================================
class Color:
    RESET = "\033[0m"
    BOLD = "\033[1m"
    DIM = "\033[2m"
    RED = "\033[31m"
    GREEN = "\033[32m"
    YELLOW = "\033[33m"
    BLUE = "\033[34m"
    MAGENTA = "\033[35m"
    CYAN = "\033[36m"
    WHITE = "\033[37m"

def cprint(text, color="", bold=False, end="\n"):
    b = Color.BOLD if bold else ""
    print(f"{b}{color}{text}{Color.RESET}", end=end)

# ==========================================
# 交互式选择辅助函数
# ==========================================
def prompt_choice(title, options, default_idx=0):
    """
    options: list of (key/desc, value)
    """
    cprint(f"\n? {title}", Color.CYAN, bold=True)
    for i, (label, _) in enumerate(options, 1):
        if i - 1 == default_idx:
            cprint(f"  [{i}] {label} (默认)", Color.GREEN, bold=True)
        else:
            cprint(f"  [{i}] {label}", Color.WHITE)
            
    while True:
        choice = input(f"请输入选项 [1-{len(options)}, 回车默认 {default_idx+1}]: ").strip()
        if not choice:
            return options[default_idx][1]
        if choice.isdigit():
            idx = int(choice) - 1
            if 0 <= idx < len(options):
                return options[idx][1]
        cprint("无效选项，请重新输入。", Color.RED)

def prompt_input(title, default=""):
    default_hint = f" [默认: {default}]" if default else ""
    cprint(f"\n? {title}{default_hint}", Color.CYAN, bold=True)
    val = input("❯ ").strip()
    return val if val else default

def prompt_yes_no(title, default=True):
    hint = "[Y/n]" if default else "[y/N]"
    cprint(f"\n? {title} {hint}", Color.CYAN, bold=True)
    val = input("❯ ").strip().lower()
    if not val:
        return default
    return val in ["y", "yes", "true", "1"]

# ==========================================
# 核心向导流程
# ==========================================
def run_wizard(last_config=None):
    cprint("\n" + "="*50, Color.BLUE, bold=True)
    cprint("🌐 交互式 HTTP 请求调试向导", Color.BLUE, bold=True)
    cprint("="*50, Color.BLUE, bold=True)

    # 1. 选择请求方式
    method_options = [
        ("GET", "GET"),
        ("POST", "POST"),
        ("PUT", "PUT"),
        ("DELETE", "DELETE"),
        ("PATCH", "PATCH"),
        ("HEAD", "HEAD")
    ]
    def_m_idx = 0
    if last_config and last_config.get("method"):
        for i, (_, m) in enumerate(method_options):
            if m == last_config["method"]:
                def_m_idx = i
                break
    method = prompt_choice("请选择请求方法 (HTTP Method):", method_options, default_idx=def_m_idx)

    # 2. 输入 URL
    default_url = (last_config.get("url") if last_config else None) or ("https://httpbin.org/get" if method == "GET" else "https://httpbin.org/post")
    url = prompt_input("请输入请求 URL:", default=default_url)
    if not url.startswith("http://") and not url.startswith("https://"):
        if url.startswith("localhost") or url.startswith("127.0.0.1"):
            url = "http://" + url
        else:
            url = "https://" + url
        cprint(f"  ℹ️ 已自动补全协议: {url}", Color.DIM)

    # 3. 配置请求头 (Headers)
    headers = {
        "User-Agent": "Interactive-CLI-HTTP/1.0",
        "Accept": "*/*"
    }

    # Content-Type 选择
    if method in ["POST", "PUT", "PATCH"]:
        ct_options = [
            ("application/json", "application/json"),
            ("application/x-www-form-urlencoded", "application/x-www-form-urlencoded"),
            ("text/plain", "text/plain"),
            ("不设置 Content-Type", None),
            ("手动自定义", "CUSTOM")
        ]
        ct = prompt_choice("请选择 Content-Type:", ct_options, default_idx=0)
        if ct == "CUSTOM":
            custom_ct = prompt_input("请输入自定义 Content-Type:")
            if custom_ct:
                headers["Content-Type"] = custom_ct
        elif ct:
            headers["Content-Type"] = ct
    else:
        # GET/DELETE 默认 Accept json
        headers["Accept"] = "application/json, text/plain, */*"

    # 补充自定义 Header
    cprint(f"\n当前预设请求头:", Color.DIM)
    for k, v in headers.items():
        cprint(f"  • {k}: {v}", Color.DIM)

    if prompt_yes_no("是否需要添加额外的请求头 (例如 Token, Cookie 等)？", default=False):
        cprint("请输入请求头 (格式: Name: Value，直接按回车结束):", Color.YELLOW)
        while True:
            line = input("Header ❯ ").strip()
            if not line:
                break
            if ":" in line:
                hk, hv = line.split(":", 1)
                headers[hk.strip()] = hv.strip()
                cprint(f"  ✅ 已添加: {hk.strip()}: {hv.strip()}", Color.GREEN)
            else:
                cprint("  ⚠️ 格式错误，请使用 'Key: Value' 格式", Color.RED)

    # 4. 请求体 (Body)
    body_data = None
    if method in ["POST", "PUT", "PATCH"]:
        body_type_options = [
            ("单行 JSON / 文本 (直接输入)", "inline"),
            ("多行文本 (多行粘贴，输入 END 结束)", "multiline"),
            ("从本地文件读取 (@file.json)", "file"),
            ("空 Body (不发送数据)", "none")
        ]
        body_type = prompt_choice("请选择请求体 (Body) 类型:", body_type_options, default_idx=0)
        
        if body_type == "inline":
            default_body = last_config.get("body", '{"msg": "hello"}') if last_config else '{"msg": "hello"}'
            body_input = prompt_input("请输入 Body 内容 (单行):", default=default_body)
            body_data = body_input.encode("utf-8") if body_input else None
        elif body_type == "multiline":
            cprint("请输入多行内容 (在最后一行单独输入 END 并回车结束):", Color.YELLOW)
            lines = []
            while True:
                line = input()
                if line.strip() == "END":
                    break
                lines.append(line)
            body_str = "\n".join(lines)
            body_data = body_str.encode("utf-8") if body_str else None
        elif body_type == "file":
            file_path = prompt_input("请输入本地文件路径:")
            p = Path(file_path).expanduser().resolve()
            if p.is_file():
                with open(p, "rb") as f:
                    body_data = f.read()
                cprint(f"  ✅ 已加载文件: {p} ({len(body_data)} 字节)", Color.GREEN)
            else:
                cprint(f"  ❌ 文件不存在: {p}，将使用空 Body", Color.RED)
                body_data = None
        else:
            body_data = None

    # 5. 生成 cURL 预览 & 确认
    curl_cmd = build_curl(method, url, headers, body_data)
    
    print("\n" + "-"*50)
    cprint("📋 即将发送的请求概况:", Color.MAGENTA, bold=True)
    cprint(f"Method : {method}", Color.WHITE, bold=True)
    cprint(f"URL    : {url}", Color.WHITE, bold=True)
    cprint("Headers:", Color.WHITE)
    for k, v in headers.items():
        cprint(f"  {k}: {v}", Color.DIM)
    if body_data:
        try:
            body_preview = body_data.decode("utf-8")
            if len(body_preview) > 300:
                body_preview = body_preview[:300] + "...(截断)"
            cprint(f"Body   : {body_preview}", Color.CYAN)
        except Exception:
            cprint(f"Body   : <二进制数据 {len(body_data)} 字节>", Color.CYAN)
    print("-"*50)
    cprint(f"等效 cURL: {curl_cmd}", Color.DIM)
    print("-"*50)

    if not prompt_yes_no("确认发送请求？", default=True):
        cprint("已取消发送。", Color.YELLOW)
        return {"method": method, "url": url, "headers": headers, "body": body_data.decode('utf-8', errors='ignore') if body_data else ""}

    # 6. 执行请求
    execute_request(method, url, headers, body_data)

    return {
        "method": method,
        "url": url,
        "headers": headers,
        "body": body_data.decode('utf-8', errors='ignore') if body_data else ""
    }

def build_curl(method, url, headers, body_data):
    parts = ["curl", "-X", method, f"'{url}'"]
    for k, v in headers.items():
        parts.append(f"-H '{k}: {v}'")
    if body_data:
        try:
            b_str = body_data.decode("utf-8")
            # 转义单引号
            b_str_escaped = b_str.replace("'", "'\\''")
            parts.append(f"-d '{b_str_escaped}'")
        except Exception:
            parts.append(f"--data-binary '<binary>'")
    return " ".join(parts)

def execute_request(method, url, headers, body_data):
    cprint("\n⏳ 正在发送请求...", Color.YELLOW)
    req = urllib.request.Request(url, data=body_data, headers=headers, method=method)
    
    # 忽略 SSL 证书验证报错 (在测试开发自签名证书时非常实用)
    ctx = ssl.create_default_context()
    ctx.check_hostname = False
    ctx.verify_mode = ssl.CERT_NONE

    start_time = time.time()
    try:
        with urllib.request.urlopen(req, context=ctx, timeout=30) as resp:
            elapsed_ms = (time.time() - start_time) * 1000
            status_code = resp.status
            resp_headers = resp.headers
            resp_bytes = resp.read()
            render_response(status_code, resp_headers, resp_bytes, elapsed_ms)
    except urllib.error.HTTPError as e:
        elapsed_ms = (time.time() - start_time) * 1000
        render_response(e.code, e.headers, e.read(), elapsed_ms)
    except Exception as e:
        elapsed_ms = (time.time() - start_time) * 1000
        cprint(f"\n❌ 请求发生异常 ({elapsed_ms:.1f}ms): {e}", Color.RED, bold=True)

def render_response(status_code, headers, body_bytes, elapsed_ms):
    # 状态码颜色
    if 200 <= status_code < 300:
        s_color = Color.GREEN
    elif 300 <= status_code < 400:
        s_color = Color.YELLOW
    else:
        s_color = Color.RED

    print("\n" + "="*50)
    cprint(f"HTTP 响应状态: {status_code}", s_color, bold=True, end="")
    cprint(f" | 耗时: {elapsed_ms:.1f} ms | 大小: {len(body_bytes)} 字节", Color.DIM)
    print("="*50)

    # 打印核心响应头
    cprint("响应头 (Response Headers):", Color.CYAN, bold=True)
    important_headers = ["content-type", "date", "server", "set-cookie", "content-length", "location"]
    for k, v in headers.items():
        if k.lower() in important_headers:
            cprint(f"  {k}: {v}", Color.DIM)
    
    # 打印响应体
    cprint("\n响应内容 (Response Body):", Color.CYAN, bold=True)
    try:
        body_str = body_bytes.decode("utf-8")
        # 尝试格式化 JSON
        try:
            parsed = json.loads(body_str)
            formatted_json = json.dumps(parsed, ensure_ascii=False, indent=2)
            cprint(formatted_json, Color.WHITE)
        except Exception:
            # 纯文本或 HTML
            if len(body_str) > 2000:
                cprint(body_str[:2000] + f"\n... [已截断，总长度 {len(body_str)} 字符]", Color.WHITE)
            else:
                cprint(body_str, Color.WHITE)
    except UnicodeDecodeError:
        cprint(f"<二进制内容或未知编码，大小: {len(body_bytes)} 字节>", Color.YELLOW)
    print("="*50)

# ==========================================
# 主入口与循环操作
# ==========================================
def main():
    last_cfg = None
    while True:
        last_cfg = run_wizard(last_config=last_cfg)
        
        post_options = [
            ("再次发送刚才的请求", "resend"),
            ("修改后重新发送 (基于刚才的数据)", "modify"),
            ("新建一个请求", "new"),
            ("退出", "exit")
        ]
        next_action = prompt_choice("请求结束，接下来你想做什么？", post_options, default_idx=3)
        
        if next_action == "resend":
            if last_cfg:
                body_bytes = last_cfg["body"].encode("utf-8") if last_cfg["body"] else None
                execute_request(last_cfg["method"], last_cfg["url"], last_cfg["headers"], body_bytes)
        elif next_action == "modify":
            continue
        elif next_action == "new":
            last_cfg = None
            continue
        elif next_action == "exit":
            cprint("\n👋 感谢使用，再见！", Color.GREEN, bold=True)
            break

if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        cprint("\n\n已退出调试工具。", Color.YELLOW)
        sys.exit(0)
