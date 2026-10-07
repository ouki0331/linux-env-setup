import os
import sys
import json
import ftplib
import paramiko
from datetime import datetime
from pathlib import Path, PurePosixPath

# ==========================================
# 工具函数
# ==========================================
def load_config(repo_path, config_file="deploy_config.json"):
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
    
    repo_name = Path(repo_path).name
    if str(repo_path) in configs:
        return configs[str(repo_path)]
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

def ensure_remote_dir_ftp(ftp, root, rel_dir):
    """递归创建 FTP 远程目录 (修复 N2)"""
    ftp.cwd(root)
    for d in PurePosixPath(rel_dir).parts:
        try:
            ftp.cwd(d)
        except ftplib.error_perm:
            ftp.mkd(d)
            ftp.cwd(d)
    ftp.cwd(root)

def ensure_remote_dir_sftp(sftp, root, rel_dir):
    """递归创建 SFTP 远程目录"""
    sftp.chdir(root)
    for d in PurePosixPath(rel_dir).parts:
        try:
            sftp.stat(d)
            sftp.chdir(d)
        except IOError:
            sftp.mkdir(d)
            sftp.chdir(d)
    sftp.chdir(root)

def ftp_size_or_none(ftp, f):
    """安全的 FTP 文件存在性与大小检查 (修复 N1)"""
    try:
        return ftp.size(f)
    except ftplib.error_perm as e:
        if not str(e).startswith("550"):
            raise
        
        parent, name = os.path.split(f)
        try:
            names = [os.path.basename(n) for n in ftp.nlst(parent or ".")]
        except ftplib.error_perm:
            names = []
            
        if name in names:
            raise RuntimeError(f"文件存在但无法取得大小 (550)，可能无权限或服务器问题: {f}") from e
        return None

# ==========================================
# 核心逻辑
# ==========================================
def process_ftp(conf, repo_path, file_list):
    backup_base = Path.home() / ".deploy_backups" / Path(repo_path).name / datetime.now().strftime('%Y%m%d_%H%M%S')
    backup_base.mkdir(parents=True, exist_ok=True)
    manifest = {}
    uploaded = []

    ftp = ftplib.FTP()
    try:
        ftp.connect(conf['host'], conf.get('port', 21), timeout=15)
        ftp.login(conf['user'], conf['pass'])
        ftp.voidcmd("TYPE I") # 强制二进制模式，避免 ASCII 模式下 SIZE 被拒 (修复 N1)
        
        ftp.cwd(conf['remote_dir'])
        real_pwd = ftp.pwd()
        
        confirm_environment("FTP", conf['host'], conf['user'], conf['remote_dir'], real_pwd, len(file_list))

        print("\n[1/2] 开始备份老文件 (FTP)...")
        for f in file_list:
            size = ftp_size_or_none(ftp, f)
            if size is None:
                print(f"➕ 检测为新文件 (远端不存在): {f}")
                manifest[f] = "new"
                continue
                
            dst = backup_base / f
            dst.parent.mkdir(parents=True, exist_ok=True)
            with open(dst, "wb") as local_f:
                ftp.retrbinary(f"RETR {f}", local_f.write)
            
            if dst.stat().st_size != size:
                raise RuntimeError(f"🛑 备份校验失败 (大小不一致): {f}")
            print(f"✅ 成功备份: {f}")
            manifest[f] = "existing"
            
        # 备份结束后立即写入 manifest (修复 N4)
        with open(backup_base / "manifest.json", "w") as mf:
            json.dump(manifest, mf, indent=2)

        print("\n[2/2] 备份完成，开始部署新文件 (FTP)...")
        for f in file_list:
            local_src = Path(repo_path) / f
            remote_dir = str(PurePosixPath(f).parent)
            
            if remote_dir and remote_dir != ".":
                ensure_remote_dir_ftp(ftp, real_pwd, remote_dir)
                
            with open(local_src, "rb") as local_f:
                ftp.storbinary(f"STOR {f}", local_f)
            print(f"🚀 成功上传: {f}")
            uploaded.append(f)
            
    except Exception as e:
        print(f"\n❌ 部署中断: {e}")
        print("以下文件已成功上传: ", uploaded)
        raise
    finally:
        try:
            ftp.quit()
        except:
            ftp.close()
            
    print(f"\n🎉 发布成功！备份已归档至: {backup_base}")

def process_sftp(conf, repo_path, file_list):
    backup_base = Path.home() / ".deploy_backups" / Path(repo_path).name / datetime.now().strftime('%Y%m%d_%H%M%S')
    backup_base.mkdir(parents=True, exist_ok=True)
    manifest = {}
    uploaded = []

    ssh = paramiko.SSHClient()
    ssh.load_system_host_keys() # 加载系统已知主机 (修复 N3)
    ssh.set_missing_host_key_policy(paramiko.RejectPolicy()) # 默认拒绝未知主机
    
    try:
        try:
            ssh.connect(hostname=conf['host'], port=conf.get('port', 22), username=conf['user'], password=conf.get('pass'), timeout=15)
        except paramiko.ssh_exception.SSHException as e:
            if "not found in known_hosts" in str(e):
                print(f"⚠️  未知主机指纹: {conf['host']}。请先通过 ssh 登录一次以信任该主机，或者设置 --trust-new-host")
            raise
            
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
            except IOError as e:
                raise RuntimeError(f"🛑 读取远端文件状态异常 (无权限或连接断开): {f}") from e

            dst = backup_base / f
            dst.parent.mkdir(parents=True, exist_ok=True)
            sftp.get(f, str(dst))
            
            if dst.stat().st_size != remote_size:
                raise RuntimeError(f"🛑 备份校验失败 (下载大小与远端不一致): {f}")
            print(f"✅ 成功备份: {f}")
            manifest[f] = "existing"
            
        # 备份结束后立即写入 manifest (修复 N4)
        with open(backup_base / "manifest.json", "w") as mf:
            json.dump(manifest, mf, indent=2)

        print("\n[2/2] 备份完成，开始部署新文件 (SFTP)...")
        for f in file_list:
            local_src = Path(repo_path) / f
            remote_dir = str(PurePosixPath(f).parent)
            
            if remote_dir and remote_dir != ".":
                ensure_remote_dir_sftp(sftp, real_pwd, remote_dir)
                
            sftp.put(str(local_src), f)
            print(f"🚀 成功上传: {f}")
            uploaded.append(f)

    except Exception as e:
        print(f"\n❌ 部署中断: {e}")
        print("以下文件已成功上传: ", uploaded)
        raise
    finally:
        try:
            sftp.close()
        except: pass
        ssh.close()
        
    print(f"\n🎉 发布成功！备份已归档至: {backup_base}")

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("用法: python deploy.py <本地仓库绝对路径> <file_list.txt 绝对路径>")
        sys.exit(1)
        
    repo_path = Path(sys.argv[1]).resolve()
    file_list_path = Path(sys.argv[2]).resolve()
    
    conf = load_config(str(repo_path))
        
    actual_files = []
    with open(file_list_path, "r") as f:
        for line in f:
            line = line.strip()
            if not line: continue
            
            p = PurePosixPath(line)
            if p.is_absolute() or ".." in p.parts:
                raise ValueError(f"🛑 非法路径，禁止相对路径逃逸或绝对路径: {line}")
            
            # 只统计真正存在的文件
            if (repo_path / line).exists():
                actual_files.append(line)
            else:
                print(f"⚠️ 忽略已删除/重命名的文件: {line}")
                
    if not actual_files:
        print("没有需要发布的有效文件。")
        sys.exit(0)
        
    if conf['protocol'].lower() == 'ftp':
        process_ftp(conf, str(repo_path), actual_files)
    elif conf['protocol'].lower() == 'sftp':
        process_sftp(conf, str(repo_path), actual_files)
    else:
        print("不支持的协议！")
