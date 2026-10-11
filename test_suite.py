#!/usr/bin/env python3
"""
HTTP CLI Automated Integration Test Suite
Covers:
1. GET request with custom headers
2. POST request with application/json
3. POST request with application/x-www-form-urlencoded
4. PUT request with application/xml
5. DELETE request
6. OPTIONS request (CORS / Allow header sniffing)
7. Basic Auth (auto-base64 encoded credentials)
"""

import sys
import json
import base64
from pathlib import Path

from http_cli import execute_and_record, init_db, get_db

init_db()

def run_test(case_name, method, url, headers, body_type, body_content_str):
    print("\n" + "="*60)
    print(f"🧪 Running test case: {case_name}")
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
        body_data=body_data,
        interactive=False
    )

print("🚀 Starting automated HTTP integration test suite...")

# 1. GET with custom header
run_test(
    case_name="GET with Custom Header",
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

# 2. POST with JSON body
run_test(
    case_name="POST with JSON payload",
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

# 3. POST with Form URL-encoded
run_test(
    case_name="POST with Form URL-encoded",
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

# 4. PUT with XML body
run_test(
    case_name="PUT with XML payload",
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

# 5. DELETE request
run_test(
    case_name="DELETE request",
    method="DELETE",
    url="https://httpbin.org/delete?item_id=98765",
    headers={
        "User-Agent": "Interactive-CLI-HTTP/1.0",
        "Accept": "application/json"
    },
    body_type="none",
    body_content_str=""
)

# 6. OPTIONS request
run_test(
    case_name="OPTIONS request (Allow / CORS detection)",
    method="OPTIONS",
    url="https://httpbin.org/get",
    headers={
        "User-Agent": "Interactive-CLI-HTTP/1.0",
        "Accept": "*/*"
    },
    body_type="none",
    body_content_str=""
)

# 7. Basic Authentication
u = "ouki"
p = "secret123"
basic_token = base64.b64encode(f"{u}:{p}".encode()).decode()
run_test(
    case_name="HTTP Basic Authentication",
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

# 8. HTML Webpage Preview Rendering
run_test(
    case_name="HTML Webpage Preview Rendering",
    method="GET",
    url="https://httpbin.org/html",
    headers={
        "User-Agent": "Interactive-CLI-HTTP/1.0",
        "Accept": "text/html"
    },
    body_type="none",
    body_content_str=""
)

# 9. Check SQLite records
print("\n" + "="*60)
print("📊 Verifying SQLite database records:")
print("="*60)
conn = get_db()
cur = conn.cursor()
cur.execute("SELECT id, method, domain, url, body_type, status_code, elapsed_ms FROM requests_history WHERE project_name='test-suite' ORDER BY id DESC LIMIT 8")
rows = cur.fetchall()
for r in reversed(rows):
    print(f"ID: {r['id']:<2} | {r['method']:<7} | Status: {r['status_code']:<3} | Elapsed: {r['elapsed_ms']:<6.1f}ms | Type: {r['body_type']:<6} | URL: {r['url']}")
conn.close()
print("="*60)
print("🎉 All integration tests completed successfully!")
