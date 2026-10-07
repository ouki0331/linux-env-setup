import os
import sys
import json
import ftplib
import paramiko
from datetime import datetime

def load_config(repo_path, config_file="deploy_config.json"):
    with open(config_file, "r") as f:
        config = json.load(f)
    return config.get(repo_path)

def confirm_environment(protocol, host, user, remote_dir):
    print("\n" + "="*50)
    print("⚠️  请核对远程服务器信息 ⚠️")
    print("="*50)
    print(f"🌍 协议类型: {protocol.upper()}")
    print(f"🖥️  远程主机: {host}")
    print(f"👤 登录账号: {user}")
    print(f"📁 远程路径: {remote_dir}")
    print("-" * 50)
    print("当前操作：即将从上述远端路径拉取老文件进行备份，并上传新文件。")
    ans = input("👉 请确认这是你要操作的环境吗？[y/N]: ")
    if ans.strip().lower() != 'y':
        print("操作已取消。")
        sys.exit(1)

def backup_and_deploy_ftp(conf, file_list):
    ftp = ftplib.FTP()
    ftp.connect(conf['host'], conf.get('port', 21))
    ftp.login(conf['user'], conf['pass'])
    
    confirm_environment("FTP", conf['host'], conf['user'], conf['remote_dir'])
    
    ftp.cwd(conf['remote_dir'])
    
    backup_dir = f"remote_backup_{datetime.now().strftime('%Y%m%d_%H%M%S')}"
    os.makedirs(backup_dir, exist_ok=True)
    
    print("\n开始通过 FTP 备份老文件...")
    for file in file_list:
        file = file.strip()
        if not file: continue
        
        # 尝试下载
        local_backup_path = os.path.join(backup_dir, file)
        os.makedirs(os.path.dirname(local_backup_path), exist_ok=True)
        try:
            with open(local_backup_path, "wb") as f:
                ftp.retrbinary(f"RETR {file}", f.write)
            print(f"✅ 成功备份: {file}")
        except ftplib.error_perm as e:
            print(f"⚠️ 远程不存在此文件 (将作为新文件上传): {file}")
            if os.path.exists(local_backup_path):
                os.remove(local_backup_path)

    print("\n开始通过 FTP 部署新文件...")
    for file in file_list:
        file = file.strip()
        if not file: continue
        
        # 简单处理远程目录创建 (简化版，生产环境需递归创建)
        remote_file_dir = os.path.dirname(file)
        if remote_file_dir:
            try:
                ftp.cwd(remote_file_dir)
                ftp.cwd(conf['remote_dir']) # 切换回根目录
            except:
                # 尝试逐层创建
                pass
                
        if os.path.exists(file):
            with open(file, "rb") as f:
                ftp.storbinary(f"STOR {file}", f)
            print(f"✅ 成功上传: {file}")
        else:
            print(f"❌ 本地找不到文件: {file}")
            
    ftp.quit()

def backup_and_deploy_sftp(conf, file_list):
    ssh = paramiko.SSHClient()
    ssh.set_missing_host_key_policy(paramiko.AutoAddPolicy())
    ssh.connect(hostname=conf['host'], port=conf.get('port', 22), username=conf['user'], password=conf.get('pass'))
    sftp = ssh.open_sftp()
    
    confirm_environment("SFTP", conf['host'], conf['user'], conf['remote_dir'])
    
    sftp.chdir(conf['remote_dir'])
    
    backup_dir = f"remote_backup_sftp_{datetime.now().strftime('%Y%m%d_%H%M%S')}"
    os.makedirs(backup_dir, exist_ok=True)
    
    print("\n开始通过 SFTP 备份老文件...")
    for file in file_list:
        file = file.strip()
        if not file: continue
        
        local_backup_path = os.path.join(backup_dir, file)
        os.makedirs(os.path.dirname(local_backup_path), exist_ok=True)
        try:
            sftp.get(file, local_backup_path)
            print(f"✅ 成功备份: {file}")
        except IOError:
            print(f"⚠️ 远程不存在此文件: {file}")

    print("\n开始通过 SFTP 部署新文件...")
    for file in file_list:
        file = file.strip()
        if not file: continue
        
        if os.path.exists(file):
            sftp.put(file, file)
            print(f"✅ 成功上传: {file}")
        else:
            print(f"❌ 本地找不到文件: {file}")
            
    sftp.close()
    ssh.close()

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: python deploy.py <repo_path> <file_list.txt>")
        sys.exit(1)
        
    repo_path = sys.argv[1]
    file_list_path = sys.argv[2]
    
    conf = load_config(repo_path)
    if not conf:
        print(f"Error: 未在配置中找到 {repo_path}")
        sys.exit(1)
        
    with open(file_list_path, "r") as f:
        files = f.readlines()
        
    if conf['protocol'].lower() == 'ftp':
        backup_and_deploy_ftp(conf, files)
    elif conf['protocol'].lower() == 'sftp':
        backup_and_deploy_sftp(conf, files)
    else:
        print("不支持的协议！")
