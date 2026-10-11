#!/usr/bin/env python3
"""
HTTP CLI 自动化综合测试套件
全面覆盖：
1. GET 请求 + 自定义 Header
2. POST 请求 + JSON 数据 (application/json)
3. POST 请求 + 表单数据 (application/x-www-form-urlencoded)
4. PUT 请求 + XML 数据 (application/xml)
5. DELETE 请求
6. OPTIONS 请求 (CORS / Allow 嗅探)
7. Basic 基础认证 (HTTP Basic Auth 用户名密码 Base64)
"""

import sys
import json
import base64
from pathlib import Path

# 导入核心逻辑
from http_cli import execute_and_record, init_db, get_db

init_db()

results = []

def run_test(case_name, method, url, headers, body_type, body_content_str):
    print("\n" + "="*60)
    print(f"🧪 正在测试用例: {case_name}")
    print(f"📡 {method} {url}")
    print("="*60)
    
    body_data = body_content_str.encode("utf-8") if body_content_str else None
    
    execute_and_record(
        project_name="test-suite",
        method=method,
        url=url,
        headers=headers,
        body_type=body_type,
        body_content_record=body_content_str,
        body_data=body_data
    )

print("🚀 开始执行全量接口自动化测试...")

# 1. GET 请求 + 自定义 Header
run_test(
    case_name="GET 请求 + 自定义 Header",
    method="GET",
    url="https://httpbin.org/get?query=test_cli",
    headers={
        "User-Agent": "Interactive-CLI-HTTP/1.0",
        "Accept": "application/json",
        "X-Custom-Client": "Ouki-HTTP-CLI"
    },
    body_type="none",
    body_content_str=""
)

# 2. POST 请求 + Content-Type: application/json
run_test(
    case_name="POST 请求 + JSON 数据",
    method="POST",
    url="https://httpbin.org/post",
    headers={
        "User-Agent": "Interactive-CLI-HTTP/1.0",
        "Content-Type": "application/json",
        "Accept": "application/json"
    },
    body_type="json",
    body_content_str='{"tool": "http_cli", "status": "active", "author": "ouki"}'
)

# 3. POST 请求 + Content-Type: application/x-www-form-urlencoded
run_test(
    case_name="POST 请求 + 表单数据 (Form)",
    method="POST",
    url="https://httpbin.org/post",
    headers={
        "User-Agent": "Interactive-CLI-HTTP/1.0",
        "Content-Type": "application/x-www-form-urlencoded",
        "Accept": "application/json"
    },
    body_type="form",
    body_content_str="username=ouki&action=login&env=wsl"
)

# 4. PUT 请求 + Content-Type: application/xml
run_test(
    case_name="PUT 请求 + XML 数据",
    method="PUT",
    url="https://httpbin.org/put",
    headers={
        "User-Agent": "Interactive-CLI-HTTP/1.0",
        "Content-Type": "application/xml",
        "Accept": "application/json"
    },
    body_type="xml",
    body_content_str="<request><user>ouki</user><role>developer</role><env>almalinux</env></request>"
)

# 5. DELETE 请求
run_test(
    case_name="DELETE 请求",
    method="DELETE",
    url="https://httpbin.org/delete?item_id=98765",
    headers={
        "User-Agent": "Interactive-CLI-HTTP/1.0",
        "Accept": "application/json"
    },
    body_type="none",
    body_content_str=""
)

# 6. OPTIONS 请求
run_test(
    case_name="OPTIONS 请求 (探测服务器支持的 HTTP 动词)",
    method="OPTIONS",
    url="https://httpbin.org/get",
    headers={
        "User-Agent": "Interactive-CLI-HTTP/1.0",
        "Accept": "*/*"
    },
    body_type="none",
    body_content_str=""
)

# 7. Basic Auth 基础认证 (用户名密码 Base64 自动生成)
u = "ouki"
p = "secret123"
basic_token = base64.b64encode(f"{u}:{p}".encode()).decode()
run_test(
    case_name="Basic Auth 基础认证",
    method="GET",
    url=f"https://httpbin.org/basic-auth/{u}/{p}",
    headers={
        "User-Agent": "Interactive-CLI-HTTP/1.0",
        "Authorization": f"Basic {basic_token}",
        "Accept": "application/json"
    },
    body_type="none",
    body_content_str=""
)

# 8. 校验 SQLite 数据库中的落盘结果
print("\n" + "="*60)
print("📊 检查 SQLite 数据库记录:")
print("="*60)
conn = get_db()
cur = conn.cursor()
cur.execute("SELECT id, method, domain, url, body_type, status_code, elapsed_ms FROM requests_history WHERE project_name='test-suite' ORDER BY id ASC")
rows = cur.fetchall()
for r in rows:
    print(f"ID: {r['id']:<2} | {r['method']:<7} | 状态: {r['status_code']:<3} | 耗时: {r['elapsed_ms']:<6.1f}ms | 类型: {r['body_type']:<6} | URL: {r['url']}")
conn.close()
print("="*60)
print("🎉 全量接口自动化测试顺利完成！")
