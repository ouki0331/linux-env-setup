#!/usr/bin/env python3
"""
交互式 CLI HTTP 调试/请求与项目管理工具 (Interactive CLI HTTP Client & Project Hub)
- 零第三方依赖：纯 Python 3 原生库 (urllib, sqlite3, ssl, json)
- 状态持久化：内置 SQLite 数据库，全生命周期记录请求参数、响应指标、时间与频次
- 项目与集合区分：按项目/文件夹归类管理请求
- 环境变量与占位符：项目级共享变量 (如 {{baseUrl}}, {{apiKey}})，支持在 URL、Header、Body 中自动替换
- 域名历史检索：按域名高效检索历史请求并支持一键回放
- 多维参数与文件上传：支持 JSON、表单、XML，支持通过 Linux/WSL 文件路径上传文件
"""

import sys
import os
import json
import time
import ssl
import sqlite3
import mimetypes
import uuid
import base64
import urllib.request
import urllib.error
import urllib.parse
from datetime import datetime
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
# 数据库管理 (SQLite)
# ==========================================
DB_PATH = Path.home() / ".http_cli.db"

def get_db():
    conn = sqlite3.connect(str(DB_PATH))
    conn.row_factory = sqlite3.Row
    return conn

def init_db():
    conn = get_db()
    with conn:
        conn.execute("""
            CREATE TABLE IF NOT EXISTS projects (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT UNIQUE NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
        """)
        conn.execute("""
            CREATE TABLE IF NOT EXISTS project_variables (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_name TEXT NOT NULL,
                var_key TEXT NOT NULL,
                var_value TEXT NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(project_name, var_key)
            )
        """)
        conn.execute("""
            CREATE TABLE IF NOT EXISTS requests_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_name TEXT NOT NULL,
                domain TEXT NOT NULL,
                method TEXT NOT NULL,
                url TEXT NOT NULL,
                headers_json TEXT,
                body_type TEXT,
                body_content TEXT,
                status_code INTEGER,
                elapsed_ms REAL,
                response_headers_json TEXT,
                response_snippet TEXT,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
        """)
        conn.execute("INSERT OR IGNORE INTO projects (name) VALUES ('default')")
    conn.close()

def list_projects():
    conn = get_db()
    cur = conn.cursor()
    cur.execute("SELECT name FROM projects ORDER BY id ASC")
    rows = cur.fetchall()
    conn.close()
    return [r['name'] for r in rows]

def create_project(name):
    conn = get_db()
    try:
        with conn:
            conn.execute("INSERT INTO projects (name) VALUES (?)", (name.strip(),))
        return True
    except sqlite3.IntegrityError:
        return False
    finally:
        conn.close()

# 环境变量管理
def get_project_variables(project_name):
    conn = get_db()
    cur = conn.cursor()
    cur.execute("SELECT var_key, var_value FROM project_variables WHERE project_name = ?", (project_name,))
    rows = cur.fetchall()
    conn.close()
    return {r['var_key']: r['var_value'] for r in rows}

def set_project_variable(project_name, key, value):
    conn = get_db()
    with conn:
        conn.execute("""
            INSERT INTO project_variables (project_name, var_key, var_value)
            VALUES (?, ?, ?)
            ON CONFLICT(project_name, var_key) DO UPDATE SET var_value = excluded.var_value
        """, (project_name, key.strip(), value.strip()))
    conn.close()

def delete_project_variable(project_name, key):
    conn = get_db()
    with conn:
        conn.execute("DELETE FROM project_variables WHERE project_name = ? AND var_key = ?", (project_name, key.strip()))
    conn.close()

def apply_variables(text, variables):
    """替换 {{var_name}} 占位符"""
    if not text or not isinstance(text, str):
        return text
    for k, v in variables.items():
        text = text.replace(f"{{{{{k}}}}}", v)
    return text

def save_request_history(project_name, method, url, headers, body_type, body_content,
                         status_code, elapsed_ms, resp_headers, resp_snippet):
    domain = urllib.parse.urlparse(url).netloc
    conn = get_db()
    with conn:
        conn.execute("""
            INSERT INTO requests_history (
                project_name, domain, method, url, headers_json,
                body_type, body_content, status_code, elapsed_ms,
                response_headers_json, response_snippet
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """, (
            project_name,
            domain,
            method,
            url,
            json.dumps(headers, ensure_ascii=False),
            body_type,
            body_content if isinstance(body_content, str) else str(body_content),
            status_code,
            elapsed_ms,
            json.dumps(dict(resp_headers), ensure_ascii=False),
            resp_snippet
        ))
    conn.close()

def query_history(domain_keyword="", project_name=None, limit=15):
    conn = get_db()
    cur = conn.cursor()
    params = []
    sql = "SELECT * FROM requests_history WHERE 1=1"
    
    if project_name:
        sql += " AND project_name = ?"
        params.append(project_name)
    if domain_keyword:
        sql += " AND domain LIKE ?"
        params.append(f"%{domain_keyword}%")
        
    sql += " ORDER BY id DESC LIMIT ?"
    params.append(limit)
    
    cur.execute(sql, params)
    rows = cur.fetchall()
    conn.close()
    return rows

# ==========================================
# 交互式选择辅助函数
# ==========================================
def prompt_choice(title, options, default_idx=0):
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
# 构建 Multipart 文件上传 Body
# ==========================================
def build_multipart_body(file_path_str, field_name="file", extra_fields=None):
    boundary = f"----HttpCliBoundary{uuid.uuid4().hex}"
    p = Path(file_path_str).expanduser().resolve()
    if not p.is_file():
        raise FileNotFoundError(f"文件不存在: {file_path_str}")

    filename = p.name
    mime_type, _ = mimetypes.guess_type(str(p))
    if not mime_type:
        mime_type = "application/octet-stream"

    with open(p, "rb") as f:
        file_bytes = f.read()

    body_bytes = bytearray()

    if extra_fields:
        for k, v in extra_fields.items():
            body_bytes.extend(f"--{boundary}\r\n".encode("utf-8"))
            body_bytes.extend(f'Content-Disposition: form-data; name="{k}"\r\n\r\n'.encode("utf-8"))
            body_bytes.extend(f"{v}\r\n".encode("utf-8"))

    body_bytes.extend(f"--{boundary}\r\n".encode("utf-8"))
    body_bytes.extend(f'Content-Disposition: form-data; name="{field_name}"; filename="{filename}"\r\n'.encode("utf-8"))
    body_bytes.extend(f"Content-Type: {mime_type}\r\n\r\n".encode("utf-8"))
    body_bytes.extend(file_bytes)
    body_bytes.extend(b"\r\n")
    body_bytes.extend(f"--{boundary}--\r\n".encode("utf-8"))

    content_type_header = f"multipart/form-data; boundary={boundary}"
    return bytes(body_bytes), content_type_header, filename

# ==========================================
# 核心向导流程
# ==========================================
def run_wizard(current_project, last_config=None):
    cprint("\n" + "="*50, Color.BLUE, bold=True)
    cprint(f"🌐 交互式 HTTP 请求向导 [当前项目: {current_project}]", Color.BLUE, bold=True)
    cprint("="*50, Color.BLUE, bold=True)

    # 加载当前项目的环境变量
    variables = get_project_variables(current_project)
    if variables:
        cprint("💡 当前项目可用环境变量 (输入 {{var}} 自动替换):", Color.YELLOW)
        for vk, vv in variables.items():
            masked_v = vv if len(vv) <= 30 else vv[:15] + "..." + vv[-6:]
            cprint(f"   • {{{{{vk}}}}} ➔ {masked_v}", Color.DIM)

    # 1. 选择请求方式
    method_options = [
        ("GET", "GET"),
        ("POST", "POST"),
        ("PUT", "PUT"),
        ("DELETE", "DELETE"),
        ("PATCH", "PATCH"),
        ("OPTIONS", "OPTIONS"),
        ("HEAD", "HEAD")
    ]
    def_m_idx = 0
    if last_config and last_config.get("method"):
        for i, (_, m) in enumerate(method_options):
            if m == last_config["method"]:
                def_m_idx = i
                break
    method = prompt_choice("请选择请求方法 (HTTP Method):", method_options, default_idx=def_m_idx)

    # 2. 输入 URL (智能预填 baseUrl)
    base_url_var = "{{baseUrl}}" if "baseUrl" in variables else None
    if base_url_var:
        default_url = (last_config.get("url") if last_config else None) or f"{base_url_var}/api/test"
    else:
        default_url = (last_config.get("url") if last_config else None) or ("https://httpbin.org/get" if method == "GET" else "https://httpbin.org/post")

    raw_url = prompt_input("请输入请求 URL (支持 {{var}} 占位符):", default=default_url)
    
    # 3. 配置请求头 (Headers) 与 身份认证 (Authentication)
    headers = {
        "User-Agent": "Interactive-CLI-HTTP/1.0",
        "Accept": "*/*"
    }

    # 身份认证配置 (支持 Basic Auth, Bearer Token)
    auth_options = [
        ("无认证 (默认)", "none"),
        ("Bearer Token (JWT / API Token)", "bearer"),
        ("Basic Auth (用户名 + 密码，自动 Base64 编码)", "basic")
    ]
    auth_choice = prompt_choice("配置身份认证 (Authentication):", auth_options, default_idx=0)
    if auth_choice == "bearer":
        def_token = "{{token}}" if "token" in variables else ("{{apiKey}}" if "apiKey" in variables else "")
        token_val = prompt_input("请输入 Bearer Token (支持 {{var}}):", default=def_token)
        if token_val:
            headers["Authorization"] = f"Bearer {token_val}"
    elif auth_choice == "basic":
        u = prompt_input("请输入 Basic Auth 用户名:")
        p = prompt_input("请输入 Basic Auth 密码:")
        token_str = base64.b64encode(f"{u}:{p}".encode("utf-8")).decode("utf-8")
        headers["Authorization"] = f"Basic {token_str}"
        cprint(f"  ✅ 已生成 Basic 认证请求头 (Base64)", Color.GREEN)

    body_data = None
    body_type = "none"
    raw_body_content = ""

    # 4. 请求体 (Body) 与 Content-Type
    if method in ["POST", "PUT", "PATCH"]:
        body_type_options = [
            ("JSON 数据 (application/json)", "json"),
            ("表单数据 (x-www-form-urlencoded)", "form"),
            ("XML / 文本数据 (application/xml 或 text/plain)", "xml"),
            ("文件上传 (multipart/form-data 或 二进制流)", "file"),
            ("空 Body (不发送数据)", "none")
        ]
        body_type = prompt_choice("请选择请求体类型 (Body Type):", body_type_options, default_idx=0)
        
        if body_type == "json":
            headers["Content-Type"] = "application/json"
            default_body = (last_config.get("body_content") if last_config and last_config.get("body_type") == "json" else '{"msg": "hello"}')
            raw_body_content = prompt_input("请输入 JSON 内容 (支持 {{var}}):", default=default_body)
        elif body_type == "form":
            headers["Content-Type"] = "application/x-www-form-urlencoded"
            cprint("请输入表单键值对 (支持 {{var}}，如 username=admin&token={{token}}):", Color.YELLOW)
            default_form = (last_config.get("body_content") if last_config and last_config.get("body_type") == "form" else 'username=admin&action=test')
            raw_body_content = prompt_input("表单数据:", default=default_form)
        elif body_type == "xml":
            ct_choice = prompt_choice("请选择具体类型:", [("application/xml", "application/xml"), ("text/plain", "text/plain")], default_idx=0)
            headers["Content-Type"] = ct_choice
            cprint("请输入多行内容 (在最后一行单独输入 END 并回车结束):", Color.YELLOW)
            lines = []
            while True:
                line = input()
                if line.strip() == "END":
                    break
                lines.append(line)
            raw_body_content = "\n".join(lines)
        elif body_type == "file":
            cprint("📁 文件上传配置 (支持绝对路径如 /home/ouki/workspace/test.png):", Color.CYAN)
            file_path = prompt_input("请输入 Linux/WSL 本地文件路径:")
            upload_mode = prompt_choice("选择上传方式:", [
                ("表单多部分上传 (multipart/form-data)", "multipart"),
                ("原始二进制流 (binary / octet-stream)", "binary")
            ], default_idx=0)

            if upload_mode == "multipart":
                field_name = prompt_input("请输入文件表单字段名称:", default="file")
                try:
                    body_data, ct_header, fname = build_multipart_body(file_path, field_name=field_name)
                    headers["Content-Type"] = ct_header
                    raw_body_content = f"[file: {file_path}] (field: {field_name}, size: {len(body_data)}B)"
                    cprint(f"  ✅ 文件已打包: {fname} ({len(body_data)} 字节)", Color.GREEN)
                except Exception as e:
                    cprint(f"  ❌ 文件处理失败: {e}，将使用空 Body", Color.RED)
                    body_data = None
            else:
                p = Path(file_path).expanduser().resolve()
                if p.is_file():
                    with open(p, "rb") as f:
                        body_data = f.read()
                    headers["Content-Type"] = "application/octet-stream"
                    raw_body_content = f"[binary_file: {file_path}] ({len(body_data)}B)"
                    cprint(f"  ✅ 二进制文件已加载: {p.name} ({len(body_data)} 字节)", Color.GREEN)
                else:
                    cprint(f"  ❌ 文件不存在: {p}", Color.RED)
                    body_data = None
        else:
            raw_body_content = ""
    else:
        headers["Accept"] = "application/json, text/plain, */*"

    # 补充自定义 Header
    cprint(f"\n当前预设请求头 (支持 {{var}} 占位符):", Color.DIM)
    for k, v in headers.items():
        cprint(f"  • {k}: {v}", Color.DIM)

    if prompt_yes_no("是否需要添加额外的请求头？", default=False):
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

    # 5. 变量求值与解析 (Interpolation)
    actual_url = apply_variables(raw_url, variables)
    if not actual_url.startswith("http://") and not actual_url.startswith("https://"):
        if actual_url.startswith("localhost") or actual_url.startswith("127.0.0.1"):
            actual_url = "http://" + actual_url
        else:
            actual_url = "https://" + actual_url

    actual_headers = {apply_variables(k, variables): apply_variables(v, variables) for k, v in headers.items()}
    
    if body_type in ["json", "form", "xml"] and raw_body_content:
        actual_body_str = apply_variables(raw_body_content, variables)
        body_data = actual_body_str.encode("utf-8")
        recorded_body_content = actual_body_str
    else:
        recorded_body_content = raw_body_content

    # 6. 生成 cURL 预览 & 概况卡片
    curl_cmd = build_curl(method, actual_url, actual_headers, body_data)
    
    print("\n" + "-"*50)
    cprint("📋 即将发送的请求概况:", Color.MAGENTA, bold=True)
    cprint(f"Project: {current_project}", Color.WHITE)
    cprint(f"Method : {method}", Color.WHITE, bold=True)
    if raw_url != actual_url:
        cprint(f"URL    : {raw_url}  ➔  {actual_url}", Color.WHITE, bold=True)
    else:
        cprint(f"URL    : {actual_url}", Color.WHITE, bold=True)
    cprint("Headers:", Color.WHITE)
    for k, v in actual_headers.items():
        cprint(f"  {k}: {v}", Color.DIM)
    if recorded_body_content:
        preview = recorded_body_content[:250] + ("..." if len(recorded_body_content) > 250 else "")
        cprint(f"Body   : ({body_type}) {preview}", Color.CYAN)
    print("-"*50)
    cprint(f"等效 cURL: {curl_cmd}", Color.DIM)
    print("-"*50)

    if not prompt_yes_no("确认发送请求？", default=True):
        cprint("已取消发送。", Color.YELLOW)
        return

    # 7. 执行请求并记录
    execute_and_record(current_project, method, actual_url, actual_headers, body_type, recorded_body_content, body_data)

def build_curl(method, url, headers, body_data):
    parts = ["curl", "-X", method, f"'{url}'"]
    for k, v in headers.items():
        parts.append(f"-H '{k}: {v}'")
    if body_data:
        try:
            b_str = body_data.decode("utf-8")
            if len(b_str) < 500:
                b_str_escaped = b_str.replace("'", "'\\''")
                parts.append(f"-d '{b_str_escaped}'")
            else:
                parts.append(f"-d '<payload {len(b_str)} bytes>'")
        except Exception:
            parts.append(f"--data-binary '<binary payload>'")
    return " ".join(parts)

def execute_and_record(project_name, method, url, headers, body_type, body_content_record, body_data):
    cprint("\n⏳ 正在发送请求...", Color.YELLOW)
    req = urllib.request.Request(url, data=body_data, headers=headers, method=method)
    
    ctx = ssl.create_default_context()
    ctx.check_hostname = False
    ctx.verify_mode = ssl.CERT_NONE

    start_time = time.time()
    status_code = 0
    resp_headers = {}
    resp_snippet = ""
    elapsed_ms = 0.0

    try:
        with urllib.request.urlopen(req, context=ctx, timeout=30) as resp:
            elapsed_ms = (time.time() - start_time) * 1000
            status_code = resp.status
            resp_headers = resp.headers
            resp_bytes = resp.read()
            resp_snippet = render_response(status_code, resp_headers, resp_bytes, elapsed_ms)
    except urllib.error.HTTPError as e:
        elapsed_ms = (time.time() - start_time) * 1000
        status_code = e.code
        resp_headers = e.headers
        resp_snippet = render_response(status_code, resp_headers, e.read(), elapsed_ms)
    except Exception as e:
        elapsed_ms = (time.time() - start_time) * 1000
        resp_snippet = f"Error: {e}"
        cprint(f"\n❌ 请求发生异常 ({elapsed_ms:.1f}ms): {e}", Color.RED, bold=True)
    finally:
        try:
            save_request_history(
                project_name=project_name,
                method=method,
                url=url,
                headers=headers,
                body_type=body_type,
                body_content=body_content_record,
                status_code=status_code,
                elapsed_ms=elapsed_ms,
                resp_headers=resp_headers,
                resp_snippet=resp_snippet[:1000]
            )
            cprint(f"💾 本次请求已记入 SQLite 数据库 (~/.http_cli.db)", Color.DIM)
        except Exception as db_err:
            cprint(f"⚠️ 记录数据库失败: {db_err}", Color.RED)

def render_response(status_code, headers, body_bytes, elapsed_ms):
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

    cprint("响应头 (Response Headers):", Color.CYAN, bold=True)
    important_headers = [
        "content-type", "date", "server", "set-cookie", "content-length", "location",
        "allow", "access-control-allow-methods", "access-control-allow-headers",
        "access-control-allow-origin", "www-authenticate"
    ]
    for k, v in headers.items():
        if k.lower() in important_headers:
            cprint(f"  {k}: {v}", Color.DIM)
    
    cprint("\n响应内容 (Response Body):", Color.CYAN, bold=True)
    snippet = ""
    try:
        body_str = body_bytes.decode("utf-8")
        snippet = body_str
        try:
            parsed = json.loads(body_str)
            formatted_json = json.dumps(parsed, ensure_ascii=False, indent=2)
            cprint(formatted_json, Color.WHITE)
        except Exception:
            if len(body_str) > 2000:
                cprint(body_str[:2000] + f"\n... [已截断，总长度 {len(body_str)} 字符]", Color.WHITE)
            else:
                cprint(body_str, Color.WHITE)
    except UnicodeDecodeError:
        snippet = f"<binary {len(body_bytes)} bytes>"
        cprint(f"<二进制内容或未知编码，大小: {len(body_bytes)} 字节>", Color.YELLOW)
    print("="*50)
    return snippet

# ==========================================
# 环境变量管理
# ==========================================
def manage_variables_flow(current_project):
    while True:
        cprint("\n" + "="*50, Color.CYAN, bold=True)
        cprint(f"⚙️  环境变量管理 [项目: {current_project}]", Color.CYAN, bold=True)
        cprint("="*50, Color.CYAN, bold=True)

        vars_dict = get_project_variables(current_project)
        if vars_dict:
            cprint("当前已定义的公共变量:", Color.GREEN)
            for k, v in vars_dict.items():
                cprint(f"  • {{{{{k}}}}} = {v}", Color.WHITE)
        else:
            cprint("当前项目暂未定义任何环境变量。", Color.DIM)

        cprint("\n操作选项:", Color.CYAN)
        options = [
            ("添加 / 修改变量 (例如 baseUrl, apiKey)", "set"),
            ("删除变量", "del"),
            ("返回主菜单", "back")
        ]
        choice = prompt_choice("请选择操作:", options, default_idx=0)

        if choice == "back":
            break
        elif choice == "set":
            cprint("请输入变量名 (不需要大括号，例如 baseUrl 或 token):", Color.YELLOW)
            key = prompt_input("变量 Key:")
            if not key:
                continue
            val = prompt_input(f"请输入 {{{{{key}}}}} 的值:")
            set_project_variable(current_project, key, val)
            cprint(f"✅ 变量 {{{{{key}}}}} 已保存！", Color.GREEN, bold=True)
        elif choice == "del":
            if not vars_dict:
                cprint("暂无可删除的变量。", Color.YELLOW)
                continue
            del_options = [(f"{{{{{k}}}}} = {v}", k) for k, v in vars_dict.items()]
            del_options.append(("取消", None))
            target_key = prompt_choice("选择要删除的变量:", del_options, default_idx=0)
            if target_key:
                delete_project_variable(current_project, target_key)
                cprint(f"✅ 变量 {{{{{target_key}}}}} 已删除！", Color.GREEN, bold=True)

# ==========================================
# 历史记录检索与回放
# ==========================================
def view_history_flow(current_project):
    cprint("\n" + "="*50, Color.CYAN, bold=True)
    cprint(f"🔍 历史请求检索与回放 [当前项目: {current_project}]", Color.CYAN, bold=True)
    cprint("="*50, Color.CYAN, bold=True)

    search_scope = prompt_choice("选择检索范围:", [
        (f"仅当前项目 ({current_project})", "current"),
        ("跨所有项目检索", "all")
    ], default_idx=0)

    proj_filter = current_project if search_scope == "current" else None
    domain_kw = prompt_input("请输入要过滤的域名关键词 (如 github.com，直接回车查看全部):", default="")

    records = query_history(domain_keyword=domain_kw, project_name=proj_filter, limit=15)
    if not records:
        cprint("没有找到符合条件的历史记录。", Color.YELLOW)
        return

    cprint(f"\n找到最近 {len(records)} 条历史记录:", Color.GREEN, bold=True)
    options = []
    for r in records:
        status_tag = f"[{r['status_code']}]" if r['status_code'] else "[ERR]"
        time_tag = r['created_at'][:16]
        desc = f"{time_tag} | {r['method']:<6} | {status_tag} | {r['domain']} | {r['url']}"
        options.append((desc, r))
    options.append(("返回上一级菜单", None))

    selected = prompt_choice("请选择要查看或回放的请求:", options, default_idx=0)
    if selected is None:
        return

    cprint("\n" + "-"*50, Color.MAGENTA, bold=True)
    cprint(f"请求时间: {selected['created_at']} | 项目: {selected['project_name']}", Color.MAGENTA)
    cprint(f"URL     : {selected['method']} {selected['url']}", Color.WHITE, bold=True)
    cprint(f"状态    : {selected['status_code']} (耗时 {selected['elapsed_ms']:.1f}ms)", Color.CYAN)
    cprint(f"参数类型: {selected['body_type']}", Color.DIM)
    cprint(f"参数内容: {selected['body_content']}", Color.WHITE)
    cprint(f"响应预览: {selected['response_snippet'][:200]}...", Color.DIM)
    print("-"*50)

    if prompt_yes_no("是否立即原样回放该请求？", default=True):
        headers = json.loads(selected['headers_json']) if selected['headers_json'] else {}
        b_type = selected['body_type']
        b_content = selected['body_content']
        body_bytes = None
        if b_content and b_type != "none":
            body_bytes = b_content.encode('utf-8')

        execute_and_record(
            project_name=current_project,
            method=selected['method'],
            url=selected['url'],
            headers=headers,
            body_type=b_type,
            body_content_record=b_content,
            body_data=body_bytes
        )

# ==========================================
# 项目管理
# ==========================================
def manage_projects_flow(current_project):
    cprint("\n" + "="*50, Color.CYAN, bold=True)
    cprint("📁 项目与集合管理", Color.CYAN, bold=True)
    cprint("="*50, Color.CYAN, bold=True)

    projects = list_projects()
    options = [(f"切换至: {p}" + (" (当前激活)" if p == current_project else ""), p) for p in projects]
    options.append(("+ 新建项目", "__NEW__"))
    options.append(("返回上一级菜单", "__BACK__"))

    choice = prompt_choice("请选择项目操作:", options, default_idx=0)
    if choice == "__BACK__":
        return current_project
    elif choice == "__NEW__":
        new_name = prompt_input("请输入新项目名称 (如 payment-api, auth-service):")
        if new_name:
            if create_project(new_name):
                cprint(f"✅ 项目 [{new_name}] 创建成功并已设为当前项目！", Color.GREEN, bold=True)
                return new_name
            else:
                cprint("⚠️ 该项目名称已存在！", Color.YELLOW)
        return current_project
    else:
        cprint(f"✅ 已切换当前项目为: [{choice}]", Color.GREEN, bold=True)
        return choice

# ==========================================
# 主入口
# ==========================================
def main():
    init_db()
    current_project = "default"

    while True:
        cprint("\n" + "="*50, Color.BLUE, bold=True)
        cprint("🌐 交互式 HTTP 调试与项目管理中枢 (http-cli)", Color.BLUE, bold=True)
        cprint(f"📁 当前激活项目: [{current_project}]", Color.MAGENTA, bold=True)
        cprint(f"💾 数据存储: SQLite ({DB_PATH})", Color.DIM)
        cprint("="*50, Color.BLUE, bold=True)

        main_options = [
            ("发起新 HTTP 请求 (向导模式)", "new_request"),
            ("浏览 / 按域名检索历史请求 (一键回放)", "history"),
            ("管理当前项目环境变量 (API Key, Base URL 等)", "variables"),
            ("切换 / 新建项目与集合", "projects"),
            ("退出", "exit")
        ]
        action = prompt_choice("请选择要执行的操作:", main_options, default_idx=0)

        if action == "new_request":
            run_wizard(current_project)
        elif action == "history":
            view_history_flow(current_project)
        elif action == "variables":
            manage_variables_flow(current_project)
        elif action == "projects":
            current_project = manage_projects_flow(current_project)
        elif action == "exit":
            cprint("\n👋 感谢使用，再见！", Color.GREEN, bold=True)
            break

if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        cprint("\n\n已退出调试工具。", Color.YELLOW)
        sys.exit(0)
