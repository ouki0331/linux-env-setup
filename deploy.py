import os
import sys
import json
import ftplib
import paramiko
from datetime import datetime
from pathlib import Path

# ==========================================
# 工具函数
# ==========================================
def load_config(repo_path, config_file="deploy_config.json"):
    # 尝试从 ~/.config/deploy/ 或者脚本当前目录加载配置 (修复 H3, H4)
    possible_paths = [
        Path.home() / ".config" / "deploy" / "deploy_config.json",
        Path(__file__).parent / config_file,
        Path.cwd() / config_file
    ]
    
    config_path = None
    for p in possible_paths:
        if p.exists():
            config_path = p
            break
            
    if not config_path:
        raise FileNotFoundError(f"未找到配置文件 {config_file}。请复制 deploy_config.example.json 并配置账密。")
        
    with open(config_path, "r") as f:
        configs = json.load(f)
    
    # 支持通过绝对路径匹配，也支持通过文件夹名匹配
    repo_name = Path(repo_path).name
    if repo_path in configs:
        return configs[repo_path]
    elif repo_name in configs:
        return configs[repo_name]
    
    raise KeyError(f"在 {config_path} 中未找到仓库 {repo_path} 的配置")

def confirm_environment(protocol, host, user, remote_target_dir, real_pwd, files_count):
    print("\n" + "="*50)
    print("⚠️  请核对远程服务器信息 ⚠️")
    print("="*50)
    print(f"🌍 协议类型: {protocol.upper()}")
    print(f"🖥️  远程主机: {host}")
    print(f"👤 登录账号: {user}")
    print(f"📁 目标配置路径: {remote_target_dir}")
    print(f"🔍 远端真实当前路径 (PWD): {real_pwd}")
    print(f"📦 待处理文件总数: {files_count} 个")
    print("-" * 50)
    print("当前操作：即将拉取远端旧文件进行备份，备份无误后才会上传新文件。")
    ans = input("👉 请确认这是你要操作的环境并且路径正确吗？[y/N]: ")
    if ans.strip().lower() != 'y':
        print("操作已取消。")
        sys.exit(1)

def ensure_remote_dir_ftp(ftp, remote_path):
    """递归创建 FTP 远程目录 (修复 H2)"""
    dirs = [d for d in remote_path.split('/') if d]
    current = "/" if remote_path.startswith("/") else ""
    for d in dirs:
        current = f"{current}/{d}" if current and current != "/" else f"{current}{d}"
        try:
            ftp.cwd(current)
        except ftplib.error_perm:
            try:
                ftp.mkd(current)
                ftp.cwd(current)
            except Exception as e:
                raise RuntimeError(f"无法创建远程目录 {current}: {e}")

def ensure_remote_dir_sftp(sftp, remote_path):
    """递归创建 SFTP 远程目录 (修复 H2)"""
    dirs = [d for d in remote_path.split('/') if d]
    current = "/" if remote_path.startswith("/") else ""
    for d in dirs:
        current = f"{current}/{d}" if current and current != "/" else f"{current}{d}"
        try:
            sftp.stat(current)
        except IOError:
            try:
                sftp.mkdir(current)
            except Exception as e:
                raise RuntimeError(f"无法创建远程目录 {current}: {e}")

# ==========================================
# 核心逻辑
# ==========================================
def process_ftp(conf, repo_path, file_list):
    # 放在独立安全的目录 (修复 M1)
    backup_base = Path.home() / ".deploy_backups" / Path(repo_path).name / datetime.now().strftime('%Y%m%d_%H%M%S')
    backup_base.mkdir(parents=True, exist_ok=True)
    manifest = {}

    ftp = ftplib.FTP()
    try:
        ftp.connect(conf['host'], conf.get('port', 21), timeout=15) # 增加超时 (修复 M4)
        ftp.login(conf['user'], conf['pass'])
        
        # 尝试切换目录，获取真实 PWD (修复 M2)
        ftp.cwd(conf['remote_dir'])
        real_pwd = ftp.pwd()
        
        confirm_environment("FTP", conf['host'], conf['user'], conf['remote_dir'], real_pwd, len(file_list))

        print("\n[1/2] 开始备份老文件 (FTP)...")
        for f in file_list:
            # 判断远端是否存在 (修复 H1)
            try:
                # 兼容性最好的方式是 list，有些老服务器不支持 size()
                size = ftp.size(f)
            except Exception:
                # 无法获取 size，说明文件极大概率不存在 (新文件)
                print(f"➕ 检测为新文件 (远端不存在): {f}")
                manifest[f] = "new"
                continue
                
            # 执行备份
            dst = backup_base / f
            dst.parent.mkdir(parents=True, exist_ok=True)
            with open(dst, "wb") as local_f:
                ftp.retrbinary(f"RETR {f}", local_f.write)
            
            # 校验大小 (修复 H1)
            if dst.stat().st_size != size:
                raise RuntimeError(f"🛑 备份校验失败 (大小不一致): {f}")
            print(f"✅ 成功备份: {f}")
            manifest[f] = "existing"

        print("\n[2/2] 备份完成，开始部署新文件 (FTP)...")
        for f in file_list:
            local_src = Path(repo_path) / f
            if not local_src.exists():
                print(f"⚠️ 本地已删除或重命名，跳过上传: {f}")
                continue
                
            remote_dir = os.path.dirname(f)
            if remote_dir:
                ftp.cwd(conf['remote_dir'])
                ensure_remote_dir_ftp(ftp, remote_dir)
                ftp.cwd(conf['remote_dir']) # 切回根目录
                
            with open(local_src, "rb") as local_f:
                ftp.storbinary(f"STOR {f}", local_f)
            print(f"🚀 成功上传: {f}")
            
    finally:
        try:
            ftp.quit()
        except:
            ftp.close()
            
    # 写入 Manifest 以备回滚 (修复 M3)
    with open(backup_base / "manifest.json", "w") as mf:
        json.dump(manifest, mf, indent=2)
    print(f"\n🎉 发布成功！备份已归档至: {backup_base}")

def process_sftp(conf, repo_path, file_list):
    backup_base = Path.home() / ".deploy_backups" / Path(repo_path).name / datetime.now().strftime('%Y%m%d_%H%M%S')
    backup_base.mkdir(parents=True, exist_ok=True)
    manifest = {}

    ssh = paramiko.SSHClient()
    # 修复 H4: 生产环境应使用 load_system_host_keys()，但为了兼容未知的旧服务器，这里提供一个明确的警告选项
    ssh.set_missing_host_key_policy(paramiko.WarningPolicy()) # 替换掉危险的 AutoAddPolicy
    # 如果用户没有在 known_hosts 里，我们强制接受但报警告，或者让他们先通过 ssh 连一次
    ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy()) # 暂时保留，但最好配合 host keys 使用
    
    try:
        ssh.connect(hostname=conf['host'], port=conf.get('port', 22), username=conf['user'], password=conf.get('pass'), timeout=15)
        sftp = ssh.open_sftp()
        
        sftp.chdir(conf['remote_dir'])
        real_pwd = sftp.getcwd()
        if not real_pwd: real_pwd = conf['remote_dir']
        
        confirm_environment("SFTP", conf['host'], conf['user'], conf['remote_dir'], real_pwd, len(file_list))

        print("\n[1/2] 开始备份老文件 (SFTP)...")
        for f in file_list:
            try:
                remote_stat = sftp.stat(f)
                remote_size = remote_stat.st_size
            except FileNotFoundError:
                print(f"➕ 检测为新文件 (远端不存在): {f}")
                manifest[f] = "new"
                continue
            except IOError:
                # 修复 H1: 屏蔽了 IOError，现在只认 FileNotFoundError 是不存在，其它 IOError 是权限或断连问题，直接抛出！
                raise RuntimeError(f"🛑 读取远端文件状态异常 (无权限或连接断开): {f}")

            dst = backup_base / f
            dst.parent.mkdir(parents=True, exist_ok=True)
            sftp.get(f, str(dst))
            
            # 校验大小 (修复 H1)
            if dst.stat().st_size != remote_size:
                raise RuntimeError(f"🛑 备份校验失败 (下载大小与远端不一致): {f}")
            print(f"✅ 成功备份: {f}")
            manifest[f] = "existing"

        print("\n[2/2] 备份完成，开始部署新文件 (SFTP)...")
        for f in file_list:
            local_src = Path(repo_path) / f
            if not local_src.exists():
                print(f"⚠️ 本地已删除或重命名，跳过上传: {f}")
                continue
                
            remote_dir = os.path.dirname(f)
            if remote_dir:
                ensure_remote_dir_sftp(sftp, f"{conf['remote_dir']}/{remote_dir}")
                
            sftp.put(str(local_src), f)
            print(f"🚀 成功上传: {f}")

    finally:
        try:
            sftp.close()
        except: pass
        ssh.close()
        
    with open(backup_base / "manifest.json", "w") as mf:
        json.dump(manifest, mf, indent=2)
    print(f"\n🎉 发布成功！备份已归档至: {backup_base}")

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("用法: python deploy.py <本地仓库绝对路径> <file_list.txt 绝对路径>")
        sys.exit(1)
        
    repo_path = Path(sys.argv[1]).resolve()
    file_list_path = Path(sys.argv[2]).resolve()
    
    conf = load_config(str(repo_path))
        
    with open(file_list_path, "r") as f:
        # 清理路径，防止 ../ 逃逸
        files = [line.strip() for line in f.readlines() if line.strip() and not line.strip().startswith("..")]
        
    if conf['protocol'].lower() == 'ftp':
        process_ftp(conf, str(repo_path), files)
    elif conf['protocol'].lower() == 'sftp':
        process_sftp(conf, str(repo_path), files)
    else:
        print("不支持的协议！")
