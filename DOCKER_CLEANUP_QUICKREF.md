# 🚀 Docker Cleanup Quick Reference

## 🔥 URGENT: Low Disk Space Fix

```bash
# Check current usage
df -h /
docker system df

# Quick safe cleanup (recommended first)
cd /workspaces/neural-trader
./scripts/docker-cleanup.sh

# If still low on space - aggressive cleanup
docker-compose down
./scripts/docker-cleanup.sh --aggressive --force
```

## 📊 Cleanup Options

| Command | What it does | Space Recovery |
|---------|--------------|----------------|
| `./scripts/docker-cleanup.sh` | Safe cleanup with prompts | 3-5GB |
| `./scripts/docker-cleanup.sh --dry-run` | Preview only, no changes | 0GB |
| `./scripts/docker-cleanup.sh --force` | Safe cleanup, no prompts | 3-5GB |
| `./scripts/docker-cleanup.sh --aggressive --force` | Full cleanup, removes everything unused | 10-15GB |
| `./scripts/docker-cleanup.sh --days 3` | Keep only last 3 days of images | 5-8GB |

## ⚡ Quick Commands

```bash
# Check disk space
df -h /

# Check Docker usage
docker system df

# Remove all stopped containers
docker container prune -f

# Remove unused images
docker image prune -a -f

# Remove build cache
docker builder prune -a -f

# Remove unused volumes (careful!)
docker volume prune -f

# Full system cleanup (nuclear option)
docker system prune -a --volumes -f
```

## 🎯 Best Practices

1. **Run cleanup weekly**: Prevents accumulation
2. **Use dry-run first**: See what will be removed
3. **Stop services before aggressive cleanup**: `docker-compose down`
4. **Check logs after**: `/tmp/docker-cleanup-*.log`

## 🆘 Emergency Recovery

If Codespace is almost full:
```bash
# 1. Stop everything
docker-compose down

# 2. Nuclear cleanup
docker system prune -a --volumes -f

# 3. Remove large files
find /workspaces -size +100M -type f

# 4. Clear package caches
rm -rf ~/.cache/*
```

---
**Script Location**: `/workspaces/neural-trader/scripts/docker-cleanup.sh`  
**Full Guide**: `/workspaces/neural-trader/DOCKER_CLEANUP_GUIDE.md`