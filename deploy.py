import os
import sys
import json
import ftplib
import paramiko
from datetime import datetime
from pathlib import Path, PurePosixPath

# ==========================================
# Utility Functions
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
        raise FileNotFoundError(f"Configuration file {config_file} not found. Please copy deploy_config.example.json and configure credentials.")
        
    with open(config_path, "r") as f:
        configs = json.load(f)
    
    repo_name = Path(repo_path).name
    if str(repo_path) in configs:
        return configs[str(repo_path)]
    elif repo_name in configs:
        return configs[repo_name]
    
    raise KeyError(f"Repository {repo_path} configuration not found in {config_path}")

def confirm_environment(protocol, host, user, remote_target_dir, real_pwd, files_count):
    print("\n" + "="*50)
    print("⚠️  Pre-flight Environment Verification ⚠️")
    print("="*50)
    print(f"Protocol : {protocol.upper()}")
    print(f"Host     : {host}")
    print(f"User     : {user}")
    print(f"Target   : {remote_target_dir}")
    print(f"Real PWD : {real_pwd}")
    print(f"Files    : {files_count} file(s)")
    print("-" * 50)
    print("Operation: Pull remote backups before safely deploying new files.")
    ans = input("👉 Confirm target environment and paths are correct? [y/N]: ")
    if ans.strip().lower() != 'y':
        print("Operation cancelled by user.")
        sys.exit(1)

def ensure_remote_dir_ftp(ftp, root, rel_dir):
    ftp.cwd(root)
    for d in PurePosixPath(rel_dir).parts:
        try:
            ftp.cwd(d)
        except ftplib.error_perm:
            ftp.mkd(d)
            ftp.cwd(d)
    ftp.cwd(root)

def ensure_remote_dir_sftp(sftp, root, rel_dir):
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
    try:
        return ftp.size(f)
    except ftplib.error_perm as e:
        err_str = str(e)
        if not (err_str.startswith("550") or err_str.startswith("500") or err_str.startswith("502")):
            raise
        
        parent, name = os.path.split(f)
        try:
            names = [os.path.basename(n) for n in ftp.nlst(parent or ".")]
        except ftplib.error_perm as nlst_e:
            if "550" in str(nlst_e):
                names = []
            else:
                raise RuntimeError(f"Directory access error (possible permission issue): {parent}") from nlst_e
            
        if name in names:
            raise RuntimeError(f"File exists but unable to obtain size: {f}") from e
        return None

# ==========================================
# Core Deployment Logic
# ==========================================
def process_ftp(conf, repo_path, file_list):
    backup_base = Path.home() / ".deploy_backups" / Path(repo_path).name / datetime.now().strftime('%Y%m%d_%H%M%S')
    manifest = {}
    uploaded = []
    current_upload = None

    ftp = ftplib.FTP()
    try:
        ftp.connect(conf['host'], conf.get('port', 21), timeout=15)
        ftp.login(conf['user'], conf['pass'])
        ftp.voidcmd("TYPE I")
        
        ftp.cwd(conf['remote_dir'])
        real_pwd = ftp.pwd()
        
        confirm_environment("FTP", conf['host'], conf['user'], conf['remote_dir'], real_pwd, len(file_list))
        backup_base.mkdir(parents=True, exist_ok=True)

        print("\n[1/2] Backing up remote files (FTP)...")
        for f in file_list:
            size = ftp_size_or_none(ftp, f)
            if size is None:
                print(f"➕ New file detected (not present on remote): {f}")
                manifest[f] = "new"
                continue
                
            dst = backup_base / f
            dst.parent.mkdir(parents=True, exist_ok=True)
            with open(dst, "wb") as local_f:
                ftp.retrbinary(f"RETR {f}", local_f.write)
            
            if size > 0 and dst.stat().st_size != size:
                raise RuntimeError(f"🛑 Backup verification failed (size mismatch): {f}")
            print(f"✅ Successfully backed up: {f}")
            manifest[f] = "existing"
            
        with open(backup_base / "manifest.json", "w") as mf:
            json.dump(manifest, mf, indent=2)

        print("\n[2/2] Backup complete. Deploying new files (FTP)...")
        for f in file_list:
            current_upload = f
            local_src = Path(repo_path) / f
            if not local_src.is_file():
                continue
                
            remote_dir = str(PurePosixPath(f).parent)
            if remote_dir and remote_dir != ".":
                ensure_remote_dir_ftp(ftp, real_pwd, remote_dir)
            
            tmp_f = f"{f}.deploy_tmp"
            with open(local_src, "rb") as local_f:
                ftp.storbinary(f"STOR {tmp_f}", local_f)
                
            tmp_size = ftp_size_or_none(ftp, tmp_f)
            if tmp_size is not None and tmp_size != local_src.stat().st_size:
                raise RuntimeError(f"🛑 Upload verification failed: {f}")
                
            try:
                ftp.delete(f)
            except: pass
            
            ftp.rename(tmp_f, f)
            print(f"🚀 Successfully uploaded: {f}")
            uploaded.append(f)
            current_upload = None
            
    except BaseException as e:
        print(f"\n❌ Deployment interrupted: {e}")
        print("Successfully uploaded files: ", uploaded)
        if current_upload:
            print(f"⚠️ Interrupted file with unknown state: {current_upload}")
        if backup_base.exists():
            with open(backup_base / "deploy_log.json", "w") as df:
                json.dump({"uploaded": uploaded, "interrupted": current_upload}, df, indent=2)
        raise
    finally:
        try:
            ftp.quit()
        except:
            ftp.close()
            
    print(f"\n🎉 Deployment successful! Backups archived to: {backup_base}")

def process_sftp(conf, repo_path, file_list):
    backup_base = Path.home() / ".deploy_backups" / Path(repo_path).name / datetime.now().strftime('%Y%m%d_%H%M%S')
    manifest = {}
    uploaded = []
    current_upload = None

    ssh = paramiko.SSHClient()
    ssh.load_system_host_keys()
    ssh.set_missing_host_key_policy(paramiko.RejectPolicy())
    
    try:
        try:
            ssh.connect(hostname=conf['host'], port=conf.get('port', 22), username=conf['user'], password=conf.get('pass'), timeout=15)
        except paramiko.ssh_exception.SSHException as e:
            if "not found in known_hosts" in str(e):
                print(f"⚠️  Unknown host key for {conf['host']}. Please SSH to this host once to trust it.")
            raise
            
        sftp = ssh.open_sftp()
        sftp.chdir(conf['remote_dir'])
        real_pwd = sftp.getcwd() or conf['remote_dir']
        
        confirm_environment("SFTP", conf['host'], conf['user'], conf['remote_dir'], real_pwd, len(file_list))
        backup_base.mkdir(parents=True, exist_ok=True)

        print("\n[1/2] Backing up remote files (SFTP)...")
        for f in file_list:
            try:
                remote_stat = sftp.stat(f)
                remote_size = remote_stat.st_size
            except FileNotFoundError:
                print(f"➕ New file detected (not present on remote): {f}")
                manifest[f] = "new"
                continue
            except IOError as e:
                raise RuntimeError(f"🛑 Error reading remote file status: {f}") from e

            dst = backup_base / f
            dst.parent.mkdir(parents=True, exist_ok=True)
            sftp.get(f, str(dst))
            
            if dst.stat().st_size != remote_size:
                raise RuntimeError(f"🛑 Backup verification failed (download size mismatch): {f}")
            print(f"✅ Successfully backed up: {f}")
            manifest[f] = "existing"
            
        with open(backup_base / "manifest.json", "w") as mf:
            json.dump(manifest, mf, indent=2)

        print("\n[2/2] Backup complete. Deploying new files (SFTP)...")
        for f in file_list:
            current_upload = f
            local_src = Path(repo_path) / f
            if not local_src.is_file():
                continue
                
            remote_dir = str(PurePosixPath(f).parent)
            if remote_dir and remote_dir != ".":
                ensure_remote_dir_sftp(sftp, real_pwd, remote_dir)
                
            tmp_f = f"{f}.deploy_tmp"
            sftp.put(str(local_src), tmp_f)
            
            if sftp.stat(tmp_f).st_size != local_src.stat().st_size:
                raise RuntimeError(f"🛑 Upload verification failed: {f}")
                
            try:
                sftp.remove(f)
            except IOError: pass
            
            sftp.posix_rename(tmp_f, f)
            print(f"🚀 Successfully uploaded: {f}")
            uploaded.append(f)
            current_upload = None

    except BaseException as e:
        print(f"\n❌ Deployment interrupted: {e}")
        print("Successfully uploaded files: ", uploaded)
        if current_upload:
            print(f"⚠️ Interrupted file with unknown state: {current_upload}")
        if backup_base.exists():
            with open(backup_base / "deploy_log.json", "w") as df:
                json.dump({"uploaded": uploaded, "interrupted": current_upload}, df, indent=2)
        raise
    finally:
        try:
            sftp.close()
        except: pass
        ssh.close()
        
    print(f"\n🎉 Deployment successful! Backups archived to: {backup_base}")

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print("Usage: python deploy.py <absolute_repo_path> <absolute_file_list.txt>")
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
                raise ValueError(f"🛑 Illegal path escape: {line}")
            
            local_target = repo_path / line
            if local_target.is_file():
                actual_files.append(line)
            elif local_target.is_dir():
                print(f"⚠️ Skipping directory: {line}")
            else:
                print(f"⚠️ Skipping deleted/missing file: {line}")
                
    if not actual_files:
        print("No valid files to deploy.")
        sys.exit(0)
        
    if conf['protocol'].lower() == 'ftp':
        process_ftp(conf, str(repo_path), actual_files)
    elif conf['protocol'].lower() == 'sftp':
        process_sftp(conf, str(repo_path), actual_files)
    else:
        print("Unsupported protocol.")
