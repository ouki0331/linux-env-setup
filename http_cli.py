#!/usr/bin/env python3
"""
Interactive CLI HTTP Client & Project Management Hub (http-cli / req)
- 100% Zero external dependencies: pure Python 3 standard library
- SQLite persistence: requests history, performance metrics, and project variables
- Project isolation: organize requests by project/collection
- Project variables: {{baseUrl}}, {{apiKey}}, {{token}} with auto-interpolation
- Instant single keypress: press 1-9 or 'b' directly without pressing Enter
- Explicit default hints: clear explanation that pressing Enter accepts the default
- Project name visibility: [Project: <name>] displayed on every step and prompt
- Steppable wizard: press 'b' at any step to return to the previous step
- Dedicated Query Parameters management (independent from URL)
- Full HTTP methods (GET, POST, PUT, DELETE, PATCH, OPTIONS, HEAD)
- Multipart file upload & Raw binary streaming from local path
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

# Terminal single-key helper
try:
    import termios
    import tty
    HAS_TERMIOS = True
except ImportError:
    HAS_TERMIOS = False

# ==========================================
# Terminal Colors & Styling
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
# Database Management (SQLite)
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

def query_history(domain_keyword="", project_name=None, limit=9):
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
# Single Keypress & Prompt Helpers
# ==========================================
def get_single_char():
    if not HAS_TERMIOS or not sys.stdin.isatty():
        line = sys.stdin.readline()
        return line[0] if line else ""
    fd = sys.stdin.fileno()
    old = termios.tcgetattr(fd)
    try:
        tty.setcbreak(fd)
        ch = sys.stdin.read(1)
    finally:
        termios.tcsetattr(fd, termios.TCSADRAIN, old)
    return ch

def prompt_instant_key(project_name, prompt_text, allowed_keys, default_key):
    proj_tag = f"[{project_name}] " if project_name else ""
    hint = "/".join([f"[{k.upper()}]" if k == default_key else k for k in allowed_keys])
    cprint(f"\n{proj_tag}? {prompt_text} ({hint})", Color.CYAN, bold=True)
    cprint(f"  ℹ️ Tip: Press key directly without Enter. Press Enter to accept default [{default_key.upper()}].", Color.DIM)
    sys.stdout.write(f"❯ ")
    sys.stdout.flush()

    while True:
        ch = get_single_char()
        if ch in ['\r', '\n']:
            print(f"{Color.GREEN}{default_key.upper()} (default){Color.RESET}")
            return default_key
        ch_lower = ch.lower()
        if ch_lower in [k.lower() for k in allowed_keys]:
            print(f"{Color.GREEN}{ch_lower}{Color.RESET}")
            return ch_lower

def prompt_choice(project_name, title, options, default_idx=0, allow_back=True):
    proj_tag = f"[{project_name}] " if project_name else ""
    cprint(f"\n{proj_tag}? {title}", Color.CYAN, bold=True)
    for i, (label, _) in enumerate(options, 1):
        if i - 1 == default_idx:
            cprint(f"  [{i}] {label} (default)", Color.GREEN, bold=True)
        else:
            cprint(f"  [{i}] {label}", Color.WHITE)
            
    if allow_back:
        cprint(f"  [b] < Back to previous step", Color.YELLOW)

    # If options count <= 9, use instant single keypress!
    use_single_key = (len(options) <= 9)
    allowed_digits = [str(i) for i in range(1, len(options) + 1)]

    back_tip = ", 'b' to go back" if allow_back else ""
    cprint(f"  ℹ️ Tip: Press 1-{len(options)} directly (no Enter needed){back_tip}, or press Enter to accept default [{default_idx+1}].", Color.DIM)
    sys.stdout.write(f"❯ ")
    sys.stdout.flush()

    if use_single_key and HAS_TERMIOS and sys.stdin.isatty():
        while True:
            ch = get_single_char()
            if ch in ['\r', '\n']:
                val = options[default_idx][1]
                print(f"{Color.GREEN}{default_idx+1} (default){Color.RESET}")
                return val
            ch_lower = ch.lower()
            if allow_back and ch_lower == 'b':
                print(f"{Color.YELLOW}b (Back){Color.RESET}")
                return "__BACK__"
            if ch_lower in allowed_digits:
                idx = int(ch_lower) - 1
                val = options[idx][1]
                print(f"{Color.GREEN}{ch_lower}{Color.RESET}")
                return val
    else:
        # Fallback for piped non-interactive input or > 9 items
        while True:
            choice = input().strip().lower()
            if not choice:
                return options[default_idx][1]
            if allow_back and choice in ["b", "back"]:
                return "__BACK__"
            if choice.isdigit():
                idx = int(choice) - 1
                if 0 <= idx < len(options):
                    return options[idx][1]
            cprint("Invalid option. Please try again.", Color.RED)
            sys.stdout.write(f"❯ ")
            sys.stdout.flush()

def prompt_input(project_name, title, default="", allow_back=True):
    proj_tag = f"[{project_name}] " if project_name else ""
    default_hint = f" [default: {default}]" if default else ""
    cprint(f"\n{proj_tag}? {title}{default_hint}", Color.CYAN, bold=True)
    back_tip = " (or type 'b' to go back)" if allow_back else ""
    cprint(f"  ℹ️ Tip: Press Enter directly to accept default{back_tip}.", Color.DIM)
    val = input("❯ ").strip()
    if allow_back and val.lower() == "b":
        return "__BACK__"
    return val if val else default

# ==========================================
# Multipart File Upload Builder
# ==========================================
def build_multipart_body(file_path_str, field_name="file", extra_fields=None):
    boundary = f"----HttpCliBoundary{uuid.uuid4().hex}"
    p = Path(file_path_str).expanduser().resolve()
    if not p.is_file():
        raise FileNotFoundError(f"File not found: {file_path_str}")

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
# Steppable Request Wizard with Back Navigation
# ==========================================
def run_wizard(current_project, last_config=None):
    cprint("\n" + "="*50, Color.BLUE, bold=True)
    cprint(f"🌐 HTTP Request Wizard [Project: {current_project}]", Color.BLUE, bold=True)
    cprint("="*50, Color.BLUE, bold=True)

    variables = get_project_variables(current_project)
    if variables:
        cprint(f"💡 [{current_project}] Available Project Variables (use {{var}} to reference):", Color.YELLOW)
        for vk, vv in variables.items():
            masked_v = vv if len(vv) <= 30 else vv[:15] + "..." + vv[-6:]
            cprint(f"   • {{{{{vk}}}}} ➔ {masked_v}", Color.DIM)

    # Wizard State
    state = {
        "method": "GET",
        "url_base": "",
        "query_params": {},
        "auth_type": "none",
        "headers": {
            "User-Agent": "Interactive-CLI-HTTP/1.0",
            "Accept": "*/*"
        },
        "body_type": "none",
        "raw_body_content": "",
        "body_data": None
    }

    step = 1
    TOTAL_STEPS = 6

    while 1 <= step <= TOTAL_STEPS:

        # ----------------------------------------------------
        # STEP 1: HTTP Method
        # ----------------------------------------------------
        if step == 1:
            method_options = [
                ("GET", "GET"),
                ("POST", "POST"),
                ("PUT", "PUT"),
                ("DELETE", "DELETE"),
                ("PATCH", "PATCH"),
                ("OPTIONS", "OPTIONS"),
                ("HEAD", "HEAD")
            ]
            cur_idx = 0
            for i, (_, m) in enumerate(method_options):
                if m == state["method"]:
                    cur_idx = i
                    break

            res = prompt_choice(current_project, "Step 1/6: Select HTTP Method:", method_options, default_idx=cur_idx, allow_back=False)
            state["method"] = res
            step += 1
            continue

        # ----------------------------------------------------
        # STEP 2: Target URL
        # ----------------------------------------------------
        elif step == 2:
            base_url_var = "{{baseUrl}}" if "baseUrl" in variables else None
            if not state["url_base"]:
                if base_url_var:
                    state["url_base"] = f"{base_url_var}/api/test"
                else:
                    state["url_base"] = "https://httpbin.org/get" if state["method"] == "GET" else "https://httpbin.org/post"

            res = prompt_input(current_project, "Step 2/6: Enter Target URL (without query parameters, or paste full URL):", default=state["url_base"])
            if res == "__BACK__":
                step -= 1
                continue

            parsed = urllib.parse.urlparse(res)
            if parsed.query:
                q_dict = dict(urllib.parse.parse_qsl(parsed.query))
                state["query_params"].update(q_dict)
                clean_url = urllib.parse.urlunparse((parsed.scheme, parsed.netloc, parsed.path, parsed.params, "", parsed.fragment))
                state["url_base"] = clean_url
            else:
                state["url_base"] = res

            step += 1
            continue

        # ----------------------------------------------------
        # STEP 3: Dedicated Query Parameters Editor
        # ----------------------------------------------------
        elif step == 3:
            cprint("\n" + "-"*40, Color.CYAN)
            cprint(f"[{current_project}] Step 3/6: Query Parameters Configuration", Color.CYAN, bold=True)
            if state["query_params"]:
                cprint("Current Parameters:", Color.GREEN)
                for k, v in state["query_params"].items():
                    cprint(f"  • {k} = {v}", Color.WHITE)
            else:
                cprint("No query parameters defined.", Color.DIM)

            param_options = [
                ("Proceed to next step (keep current parameters)", "next"),
                ("Add / Update query parameter (key=value)", "add"),
                ("Remove a query parameter", "remove"),
                ("Clear all parameters", "clear")
            ]
            p_action = prompt_choice(current_project, "Parameter Options:", param_options, default_idx=0)
            if p_action == "__BACK__":
                step -= 1
                continue
            elif p_action == "next":
                step += 1
                continue
            elif p_action == "add":
                param_key = prompt_input(current_project, "Parameter Key:", allow_back=False)
                if param_key:
                    param_val = prompt_input(current_project, f"Value for '{param_key}':", allow_back=False)
                    state["query_params"][param_key] = param_val
                    cprint(f"  ✅ Added parameter: {param_key}={param_val}", Color.GREEN)
                continue
            elif p_action == "remove":
                if not state["query_params"]:
                    cprint("No parameters to remove.", Color.YELLOW)
                    continue
                rem_options = [(f"{k} = {v}", k) for k, v in state["query_params"].items()]
                rem_key = prompt_choice(current_project, "Select parameter to remove:", rem_options, default_idx=0)
                if rem_key != "__BACK__":
                    state["query_params"].pop(rem_key, None)
                    cprint(f"  ✅ Removed parameter: {rem_key}", Color.GREEN)
                continue
            elif p_action == "clear":
                state["query_params"].clear()
                cprint("  ✅ Cleared all parameters.", Color.GREEN)
                continue

        # ----------------------------------------------------
        # STEP 4: Authentication & Headers
        # ----------------------------------------------------
        elif step == 4:
            cprint("\n" + "-"*40, Color.CYAN)
            cprint(f"[{current_project}] Step 4/6: Authentication & Headers", Color.CYAN, bold=True)

            auth_options = [
                ("No Authentication (default)", "none"),
                ("Bearer Token (JWT / API Token)", "bearer"),
                ("Basic Auth (Username + Password with Auto-Base64)", "basic")
            ]
            auth_res = prompt_choice(current_project, "Configure Authentication:", auth_options, default_idx=0)
            if auth_res == "__BACK__":
                step -= 1
                continue

            state["auth_type"] = auth_res
            if auth_res == "bearer":
                token_def = "{{token}}" if "token" in variables else ("{{apiKey}}" if "apiKey" in variables else "")
                token_val = prompt_input(current_project, "Enter Bearer Token (supports {{var}}):", default=token_def, allow_back=False)
                if token_val:
                    state["headers"]["Authorization"] = f"Bearer {token_val}"
            elif auth_res == "basic":
                u = prompt_input(current_project, "Basic Auth Username:", allow_back=False)
                p = prompt_input(current_project, "Basic Auth Password:", allow_back=False)
                token_str = base64.b64encode(f"{u}:{p}".encode("utf-8")).decode("utf-8")
                state["headers"]["Authorization"] = f"Basic {token_str}"
                cprint("  ✅ Basic Auth header generated.", Color.GREEN)
            else:
                state["headers"].pop("Authorization", None)

            cprint(f"\n[{current_project}] Current Request Headers:", Color.DIM)
            for k, v in state["headers"].items():
                cprint(f"  • {k}: {v}", Color.DIM)

            add_h_key = prompt_instant_key(current_project, "Add custom headers? (e.g. X-Trace-Id)", ["y", "n", "b"], default_key="n")
            if add_h_key == "b":
                step -= 1
                continue
            elif add_h_key == "y":
                cprint("Enter headers in 'Key: Value' format (blank line to finish):", Color.YELLOW)
                while True:
                    line = input("Header ❯ ").strip()
                    if not line:
                        break
                    if ":" in line:
                        hk, hv = line.split(":", 1)
                        state["headers"][hk.strip()] = hv.strip()
                        cprint(f"  ✅ Added: {hk.strip()}: {hv.strip()}", Color.GREEN)
                    else:
                        cprint("  ⚠️ Invalid format. Please use 'Key: Value'", Color.RED)

            step += 1
            continue

        # ----------------------------------------------------
        # STEP 5: Request Body (if POST/PUT/PATCH/DELETE)
        # ----------------------------------------------------
        elif step == 5:
            if state["method"] in ["GET", "HEAD", "OPTIONS"]:
                state["body_type"] = "none"
                state["raw_body_content"] = ""
                state["body_data"] = None
                step += 1
                continue

            cprint("\n" + "-"*40, Color.CYAN)
            cprint(f"[{current_project}] Step 5/6: Request Body Configuration ({state['method']})", Color.CYAN, bold=True)

            body_type_options = [
                ("JSON (application/json)", "json"),
                ("Form URL-Encoded (application/x-www-form-urlencoded)", "form"),
                ("XML / Plain Text", "xml"),
                ("File Upload (Multipart / Binary)", "file"),
                ("Empty Body (No payload)", "none")
            ]
            b_choice = prompt_choice(current_project, "Select Body Type:", body_type_options, default_idx=0)
            if b_choice == "__BACK__":
                step -= 1
                continue

            state["body_type"] = b_choice
            if b_choice == "json":
                state["headers"]["Content-Type"] = "application/json"
                def_json = state["raw_body_content"] or '{"msg": "hello"}'
                json_in = prompt_input(current_project, "Enter JSON Payload (supports {{var}}):", default=def_json)
                if json_in == "__BACK__":
                    continue
                state["raw_body_content"] = json_in
            elif b_choice == "form":
                state["headers"]["Content-Type"] = "application/x-www-form-urlencoded"
                def_form = state["raw_body_content"] or "username=admin&action=test"
                form_in = prompt_input(current_project, "Enter Form Data (key=value&...):", default=def_form)
                if form_in == "__BACK__":
                    continue
                state["raw_body_content"] = form_in
            elif b_choice == "xml":
                ct_sub = prompt_choice(current_project, "Choose Specific Content-Type:", [("application/xml", "application/xml"), ("text/plain", "text/plain")], default_idx=0)
                if ct_sub == "__BACK__":
                    continue
                state["headers"]["Content-Type"] = ct_sub
                cprint("Enter multi-line text (type 'END' on a single line to finish):", Color.YELLOW)
                lines = []
                while True:
                    line = input()
                    if line.strip() == "END":
                        break
                    lines.append(line)
                state["raw_body_content"] = "\n".join(lines)
            elif b_choice == "file":
                file_path = prompt_input(current_project, "Enter Local File Path (Linux/WSL path):")
                if file_path == "__BACK__":
                    continue
                upload_mode = prompt_choice(current_project, "Select Upload Mode:", [
                    ("Multipart Form-Data (standard web uploads)", "multipart"),
                    ("Raw Binary Octet-Stream", "binary")
                ], default_idx=0)
                if upload_mode == "__BACK__":
                    continue

                if upload_mode == "multipart":
                    field_name = prompt_input(current_project, "Enter Form Field Name:", default="file", allow_back=False)
                    try:
                        b_bytes, ct_header, fname = build_multipart_body(file_path, field_name=field_name)
                        state["headers"]["Content-Type"] = ct_header
                        state["body_data"] = b_bytes
                        state["raw_body_content"] = f"[file: {file_path}] (field: {field_name}, {len(b_bytes)}B)"
                        cprint(f"  ✅ Packaged file: {fname} ({len(b_bytes)} bytes)", Color.GREEN)
                    except Exception as e:
                        cprint(f"  ❌ File packaging failed: {e}", Color.RED)
                        state["body_data"] = None
                else:
                    p = Path(file_path).expanduser().resolve()
                    if p.is_file():
                        with open(p, "rb") as f:
                            state["body_data"] = f.read()
                        state["headers"]["Content-Type"] = "application/octet-stream"
                        state["raw_body_content"] = f"[binary_file: {file_path}] ({len(state['body_data'])}B)"
                        cprint(f"  ✅ Loaded binary file ({len(state['body_data'])} bytes)", Color.GREEN)
                    else:
                        cprint(f"  ❌ File not found: {p}", Color.RED)
                        state["body_data"] = None
            else:
                state["raw_body_content"] = ""
                state["body_data"] = None

            step += 1
            continue

        # ----------------------------------------------------
        # STEP 6: Pre-send Summary & Instant Single-Key Confirmation
        # ----------------------------------------------------
        elif step == 6:
            raw_url_full = state["url_base"]
            if state["query_params"]:
                qs = urllib.parse.urlencode(state["query_params"])
                sep = "&" if "?" in raw_url_full else "?"
                raw_url_full = f"{raw_url_full}{sep}{qs}"

            actual_url = apply_variables(raw_url_full, variables)
            if not actual_url.startswith("http://") and not actual_url.startswith("https://"):
                if actual_url.startswith("localhost") or actual_url.startswith("127.0.0.1"):
                    actual_url = "http://" + actual_url
                else:
                    actual_url = "https://" + actual_url

            actual_headers = {apply_variables(k, variables): apply_variables(v, variables) for k, v in state["headers"].items()}

            if state["body_type"] in ["json", "form", "xml"] and state["raw_body_content"]:
                actual_body_str = apply_variables(state["raw_body_content"], variables)
                body_payload = actual_body_str.encode("utf-8")
                recorded_content = actual_body_str
            else:
                body_payload = state["body_data"]
                recorded_content = state["raw_body_content"]

            curl_cmd = build_curl(state["method"], actual_url, actual_headers, body_payload)

            print("\n" + "="*50)
            cprint(f"📋 [{current_project}] Step 6/6: Request Overview Card", Color.MAGENTA, bold=True)
            cprint(f"Project : {current_project}", Color.WHITE)
            cprint(f"Method  : {state['method']}", Color.WHITE, bold=True)
            if raw_url_full != actual_url:
                cprint(f"URL (tpl): {raw_url_full}", Color.DIM)
                cprint(f"URL (raw): {actual_url}", Color.WHITE, bold=True)
            else:
                cprint(f"URL     : {actual_url}", Color.WHITE, bold=True)
            
            cprint("Headers :", Color.WHITE)
            for k, v in actual_headers.items():
                cprint(f"  {k}: {v}", Color.DIM)

            if recorded_content:
                preview = recorded_content[:200] + ("..." if len(recorded_content) > 200 else "")
                cprint(f"Body ({state['body_type']}): {preview}", Color.CYAN)
            print("="*50)
            cprint(f"Equivalent cURL: {curl_cmd}", Color.DIM)
            print("="*50)

            key_choice = prompt_instant_key(current_project, "Send request now? (y=Send, n=Cancel, b=Back)", ["y", "n", "b"], default_key="y")
            if key_choice == "b":
                step -= 1
                continue
            elif key_choice == "n":
                cprint("Request cancelled by user.", Color.YELLOW)
                return

            execute_and_record(
                project_name=current_project,
                method=state["method"],
                url=actual_url,
                headers=actual_headers,
                body_type=state["body_type"],
                body_content_record=recorded_content,
                body_data=body_payload
            )
            step += 1
            break

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
    cprint(f"\n⏳ [{project_name}] Sending HTTP request...", Color.YELLOW)
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
        cprint(f"\n❌ [{project_name}] Request failed ({elapsed_ms:.1f}ms): {e}", Color.RED, bold=True)
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
            cprint(f"💾 [{project_name}] Request logged to SQLite database (~/.http_cli.db)", Color.DIM)
        except Exception as db_err:
            cprint(f"⚠️ Failed to log to SQLite: {db_err}", Color.RED)

def render_response(status_code, headers, body_bytes, elapsed_ms):
    if 200 <= status_code < 300:
        s_color = Color.GREEN
    elif 300 <= status_code < 400:
        s_color = Color.YELLOW
    else:
        s_color = Color.RED

    print("\n" + "="*50)
    cprint(f"HTTP Status: {status_code}", s_color, bold=True, end="")
    cprint(f" | Elapsed: {elapsed_ms:.1f} ms | Size: {len(body_bytes)} bytes", Color.DIM)
    print("="*50)

    cprint("Response Headers:", Color.CYAN, bold=True)
    important_headers = [
        "content-type", "date", "server", "set-cookie", "content-length", "location",
        "allow", "access-control-allow-methods", "access-control-allow-headers",
        "access-control-allow-origin", "www-authenticate"
    ]
    for k, v in headers.items():
        if k.lower() in important_headers:
            cprint(f"  {k}: {v}", Color.DIM)
    
    cprint("\nResponse Body:", Color.CYAN, bold=True)
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
                cprint(body_str[:2000] + f"\n... [Truncated, total length: {len(body_str)} chars]", Color.WHITE)
            else:
                cprint(body_str, Color.WHITE)
    except UnicodeDecodeError:
        snippet = f"<binary {len(body_bytes)} bytes>"
        cprint(f"<Binary content or unknown encoding, size: {len(body_bytes)} bytes>", Color.YELLOW)
    print("="*50)
    return snippet

# ==========================================
# Project Variables Management
# ==========================================
def manage_variables_flow(current_project):
    while True:
        cprint("\n" + "="*50, Color.CYAN, bold=True)
        cprint(f"⚙️  Project Variables [Project: {current_project}]", Color.CYAN, bold=True)
        cprint("="*50, Color.CYAN, bold=True)

        vars_dict = get_project_variables(current_project)
        if vars_dict:
            cprint(f"[{current_project}] Defined Public Variables:", Color.GREEN)
            for k, v in vars_dict.items():
                cprint(f"  • {{{{{k}}}}} = {v}", Color.WHITE)
        else:
            cprint(f"[{current_project}] No variables defined yet.", Color.DIM)

        options = [
            ("Add / Update Variable (e.g. baseUrl, apiKey)", "set"),
            ("Delete Variable", "del"),
            ("Back to Main Menu", "back")
        ]
        choice = prompt_choice(current_project, "Select Variable Action:", options, default_idx=0, allow_back=False)

        if choice == "back":
            break
        elif choice == "set":
            key = prompt_input(current_project, "Variable Name (e.g. baseUrl, token):", allow_back=False)
            if not key:
                continue
            val = prompt_input(current_project, f"Value for {{{{{key}}}}}:", allow_back=False)
            set_project_variable(current_project, key, val)
            cprint(f"✅ [{current_project}] Saved variable {{{{{key}}}}} = {val}", Color.GREEN, bold=True)
        elif choice == "del":
            if not vars_dict:
                cprint(f"[{current_project}] No variables to delete.", Color.YELLOW)
                continue
            del_options = [(f"{{{{{k}}}}} = {v}", k) for k, v in vars_dict.items()]
            del_options.append(("Cancel", None))
            target_key = prompt_choice(current_project, "Select variable to delete:", del_options, default_idx=0, allow_back=False)
            if target_key:
                delete_project_variable(current_project, target_key)
                cprint(f"✅ [{current_project}] Deleted variable {{{{{target_key}}}}}", Color.GREEN, bold=True)

# ==========================================
# History Search & Instant Replay
# ==========================================
def view_history_flow(current_project):
    cprint("\n" + "="*50, Color.CYAN, bold=True)
    cprint(f"🔍 History Search & Replay [Project: {current_project}]", Color.CYAN, bold=True)
    cprint("="*50, Color.CYAN, bold=True)

    search_scope = prompt_choice(current_project, "Search Scope:", [
        (f"Current Project only ({current_project})", "current"),
        ("All Projects", "all")
    ], default_idx=0, allow_back=False)

    proj_filter = current_project if search_scope == "current" else None
    domain_kw = prompt_input(current_project, "Filter by domain keyword (e.g. github.com, Enter for all):", default="", allow_back=False)

    records = query_history(domain_keyword=domain_kw, project_name=proj_filter, limit=9)
    if not records:
        cprint(f"[{current_project}] No matching history records found.", Color.YELLOW)
        return

    cprint(f"\n[{current_project}] Found {len(records)} recent records:", Color.GREEN, bold=True)
    options = []
    for r in records:
        status_tag = f"[{r['status_code']}]" if r['status_code'] else "[ERR]"
        time_tag = r['created_at'][:16]
        desc = f"{time_tag} | {r['method']:<6} | {status_tag} | {r['domain']}"
        options.append((desc, r))
    options.append(("Back to Main Menu", None))

    selected = prompt_choice(current_project, "Select a request to inspect / replay:", options, default_idx=0, allow_back=False)
    if selected is None:
        return

    cprint("\n" + "-"*50, Color.MAGENTA, bold=True)
    cprint(f"Timestamp : {selected['created_at']} | Project: {selected['project_name']}", Color.MAGENTA)
    cprint(f"Endpoint  : {selected['method']} {selected['url']}", Color.WHITE, bold=True)
    cprint(f"Status    : {selected['status_code']} ({selected['elapsed_ms']:.1f}ms)", Color.CYAN)
    cprint(f"Body Type : {selected['body_type']}", Color.DIM)
    cprint(f"Payload   : {selected['body_content']}", Color.WHITE)
    cprint(f"Response  : {selected['response_snippet'][:200]}...", Color.DIM)
    print("-"*50)

    replay_key = prompt_instant_key(current_project, "Replay this request now? (y/n)", ["y", "n"], default_key="y")
    if replay_key == "y":
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
# Projects Management
# ==========================================
def manage_projects_flow(current_project):
    cprint("\n" + "="*50, Color.CYAN, bold=True)
    cprint(f"📁 Projects & Collections [Active: {current_project}]", Color.CYAN, bold=True)
    cprint("="*50, Color.CYAN, bold=True)

    projects = list_projects()
    options = [(f"{p}" + (" (ACTIVE)" if p == current_project else ""), p) for p in projects]
    options.append(("+ Create New Project", "__NEW__"))
    options.append(("Back to Main Menu", "__BACK__"))

    choice = prompt_choice(current_project, "Select Project Action:", options, default_idx=0, allow_back=False)
    if choice == "__BACK__":
        return current_project
    elif choice == "__NEW__":
        new_name = prompt_input(current_project, "Enter new project name (e.g. payment-service, auth-api):", allow_back=False)
        if new_name:
            if create_project(new_name):
                cprint(f"✅ Created project [{new_name}] and set as active!", Color.GREEN, bold=True)
                return new_name
            else:
                cprint("⚠️ Project name already exists!", Color.YELLOW)
        return current_project
    else:
        cprint(f"✅ Switched active project to: [{choice}]", Color.GREEN, bold=True)
        return choice

# ==========================================
# Main CLI Entrypoint
# ==========================================
def main():
    init_db()
    current_project = "default"

    while True:
        cprint("\n" + "="*50, Color.BLUE, bold=True)
        cprint("🌐 Interactive HTTP Client & Hub (http-cli / req)", Color.BLUE, bold=True)
        cprint(f"📁 Active Project: [{current_project}]", Color.MAGENTA, bold=True)
        cprint(f"💾 Storage: SQLite ({DB_PATH})", Color.DIM)
        cprint("="*50, Color.BLUE, bold=True)

        main_options = [
            ("Send New HTTP Request (Wizard)", "new_request"),
            ("Search History by Domain (Replay)", "history"),
            ("Project Variables (baseUrl, apiKey, token)", "variables"),
            ("Projects & Collections", "projects"),
            ("Exit", "exit")
        ]
        action = prompt_choice(current_project, "Select Main Action:", main_options, default_idx=0, allow_back=False)

        if action == "new_request":
            run_wizard(current_project)
        elif action == "history":
            view_history_flow(current_project)
        elif action == "variables":
            manage_variables_flow(current_project)
        elif action == "projects":
            current_project = manage_projects_flow(current_project)
        elif action == "exit":
            cprint(f"\n👋 [{current_project}] Goodbye!", Color.GREEN, bold=True)
            break

if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        cprint("\n\nExited http-cli.", Color.YELLOW)
        sys.exit(0)
