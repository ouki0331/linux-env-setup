import os
import sys
import json
import ftplib
import paramiko
from pathlib import Path

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
        raise FileNotFoundError(f"未找到配置文件 {config_file}")
        
    with open(config_path, "r") as f:
        configs = json.load(f)
    
    repo_name = Path(repo_path).name
    if str(repo_path) in configs:
        return configs[str(repo_path)]
    elif repo_name in configs:
        return configs[repo_name]
    raise KeyError(f"未找到配置")

def rollback_ftp(conf, backup_dir, manifest):
    ftp = ftplib.FTP()
    try:
        ftp.connect(conf['host'], conf.get('port', 21), timeout=15)
        ftp.login(conf['user'], conf['pass'])
        ftp.cwd(conf['remote_dir'])
        ftp.voidcmd("TYPE I")
        
        print("\n开始执行 FTP 回滚...")
        for file, status in manifest.items():
            if status == "new":
                print(f"🗑️  删除意外发布的新文件: {file}")
                try:
                    ftp.delete(file)
                except Exception as e:
                    print(f"   (删除失败，可能远端已不存在: {e})")
            elif status == "existing":
                print(f"🔄 恢复老文件: {file}")
                local_src = backup_dir / file
                with open(local_src, "rb") as local_f:
                    ftp.storbinary(f"STOR {file}", local_f)
                    
        print("\n🎉 回滚完成！")
    finally:
        try:
            ftp.quit()
        except:
            pass

def rollback_sftp(conf, backup_dir, manifest):
    ssh = paramiko.SSHClient()
    ssh.load_system_host_keys()
    ssh.set_missing_host_key_policy(paramiko.RejectPolicy())
    try:
        ssh.connect(hostname=conf['host'], port=conf.get('port', 22), username=conf['user'], password=conf.get('pass'), timeout=15)
        sftp = ssh.open_sftp()
        sftp.chdir(conf['remote_dir'])
        
        print("\n开始执行 SFTP 回滚...")
        for file, status in manifest.items():
            if status == "new":
                print(f"🗑️  删除意外发布的新文件: {file}")
                try:
                    sftp.remove(file)
                except Exception as e:
                    print(f"   (删除失败，可能远端已不存在: {e})")
            elif status == "existing":
                print(f"🔄 恢复老文件: {file}")
                local_src = backup_dir / file
                sftp.put(str(local_src), file)
                
        print("\n🎉 回滚完成！")
    finally:
        try:
            sftp.close()
        except: pass
        ssh.close()

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("用法: python rollback.py <本地仓库绝对路径> <备份文件夹绝对路径>")
        sys.exit(1)
        
    repo_path = Path(sys.argv[1]).resolve()
    backup_dir = Path(sys.argv[2]).resolve()
    manifest_path = backup_dir / "manifest.json"
    
    if not manifest_path.exists():
        print(f"🛑 在 {backup_dir} 找不到 manifest.json，无法确认回滚策略。")
        sys.exit(1)
        
    with open(manifest_path, "r") as f:
        manifest = json.load(f)
        
    conf = load_config(str(repo_path))
    
    ans = input(f"👉 确认将 {repo_path.name} 恢复到 {backup_dir.name} 的状态吗？[y/N]: ")
    if ans.strip().lower() != 'y':
        sys.exit(1)
        
    if conf['protocol'].lower() == 'ftp':
        rollback_ftp(conf, backup_dir, manifest)
    elif conf['protocol'].lower() == 'sftp':
        rollback_sftp(conf, backup_dir, manifest)
