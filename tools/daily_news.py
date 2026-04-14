#!/usr/bin/env python3
import argparse
import urllib.request
import urllib.parse
import json
from datetime import datetime

BILIBILI_API = "https://api.bilibili.com/x/web-interface/popular"
DOUBAN_MOVIE_URL = "https://movie.douban.com/j/search_subjects"
DOUBAN_BOOK_URL = "https://book.douban.com/j/search_subjects"

def fetch_bilibili_trending(limit=10):
    try:
        url = f"{BILIBILI_API}?pn=1&ps={limit}"
        req = urllib.request.Request(url, headers={
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Referer": "https://www.bilibili.com"
        })
        with urllib.request.urlopen(req, timeout=10) as response:
            data = json.loads(response.read().decode())
        
        items = []
        if data.get("code") == 0:
            for item in data.get("data", {}).get("list", [])[:limit]:
                items.append({
                    "title": item.get("title", ""),
                    "desc": item.get("desc", ""),
                    "hot": item.get("stat", {}).get("view", 0),
                    "link": f"https://www.bilibili.com/video/{item.get('bvid', '')}"
                })
        return items
    except Exception as e:
        return [{"error": str(e)}]

def fetch_bilibili_live_trending(limit=10):
    try:
        url = "https://api.bilibili.com/x/web-interface/ranking/v2?rid=0&type=hot"
        req = urllib.request.Request(url, headers={
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            "Referer": "https://www.bilibili.com"
        })
        with urllib.request.urlopen(req, timeout=10) as response:
            data = json.loads(response.read().decode())
        
        items = []
        if data.get("code") == 0:
            for item in data.get("data", {}).get("list", [])[:limit]:
                items.append({
                    "title": item.get("title", ""),
                    "desc": item.get("desc", ""),
                    "hot": item.get("stat", {}).get("view", 0),
                    "link": f"https://www.bilibili.com/video/{item.get('bvid', '')}"
                })
        return items
    except Exception as e:
        return [{"error": str(e)}]

def fetch_douban_movie_trending(limit=10):
    try:
        url = f"{DOUBAN_MOVIE_URL}?type=tv&tag=%E7%83%AD%E9%97%A8&page_limit={limit}"
        req = urllib.request.Request(url, headers={
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
        })
        with urllib.request.urlopen(req, timeout=10) as response:
            data = json.loads(response.read().decode())
        
        items = []
        for item in data.get("subjects", [])[:limit]:
            items.append({
                "title": item.get("title", ""),
                "desc": item.get("episodes_info", ""),
                "rate": item.get("rate", ""),
                "hot": item.get("cover", ""),
                "link": item.get("url", "")
            })
        return items
    except Exception as e:
        return [{"error": str(e)}]

def fetch_douban_movie_showing(limit=10):
    try:
        url = f"{DOUBAN_MOVIE_URL}?type=tv&tag=%E7%8E%AF%E5%BD%B1&page_limit={limit}"
        req = urllib.request.Request(url, headers={
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
        })
        with urllib.request.urlopen(req, timeout=10) as response:
            data = json.loads(response.read().decode())
        
        items = []
        for item in data.get("subjects", [])[:limit]:
            items.append({
                "title": item.get("title", ""),
                "desc": item.get("episodes_info", ""),
                "rate": item.get("rate", ""),
                "link": item.get("url", "")
            })
        return items
    except Exception as e:
        return [{"error": str(e)}]

def fetch_github_trending(limit=10):
    try:
        url = "https://github.com/trending?since=weekly"
        req = urllib.request.Request(url, headers={
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
        })
        with urllib.request.urlopen(req, timeout=10) as response:
            html = response.read().decode()
        
        import re
        items = []
        pattern = r'<h2[^>]*class="[^"]*"[^>]*><a[^>]*href="([^"]+)"[^>]*>([^<]+)</a>'
        matches = re.findall(pattern, html)[:limit]
        for link, title in matches:
            items.append({
                "title": title.strip(),
                "link": f"https://github.com{link}",
                "desc": ""
            })
        if not items:
            pattern2 = r'<a[^>]*href="/([^"/]+/[^"/]+)"[^>]*>([^<]+)</a>'
            matches2 = re.findall(pattern2, html)
            for link, title in matches2[:limit]:
                items.append({
                    "title": title.strip(),
                    "link": f"https://github.com/{link}",
                    "desc": ""
                })
        return items if items else [{"error": "No items found"}]
    except Exception as e:
        return [{"error": str(e)}]

def fetch_zhihu_questions(limit=10):
    try:
        url = "https://www.zhihu.com/api/v3/feed/topstory/hot-lists/total?limit=50"
        req = urllib.request.Request(url, headers={
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        })
        with urllib.request.urlopen(req, timeout=10) as response:
            data = json.loads(response.read().decode())
        
        items = []
        for item in data.get("data", [])[:limit]:
            items.append({
                "title": item.get("target", {}).get("title", ""),
                "detail": item.get("detail_text", ""),
                "link": f"https://www.zhihu.com/question/{item.get('target', {}).get('id', '')}"
            })
        return items
    except Exception as e:
        return [{"error": str(e)}]

LOCATION_SOURCE_MAP = {
    "bilibili": "bilibili",
    "b站": "bilibili",
    "bili": "bilibili",
    "bilihot": "bilibili_live",
    "douban": "douban_movie",
    "豆瓣": "douban_movie",
    "movie": "douban_movie",
    "tv": "douban_showing",
    "github": "github",
    "zhihu": "zhihu",
}

SOURCE_FETCHERS = {
    "bilibili": ("B站热门", fetch_bilibili_trending),
    "bilibili_live": ("B站排行榜", fetch_bilibili_live_trending),
    "douban_movie": ("豆瓣热门电视剧", fetch_douban_movie_trending),
    "douban_showing": ("豆瓣热门综艺", fetch_douban_movie_showing),
    "github": ("GitHub趋势", fetch_github_trending),
}

def format_number(num):
    if isinstance(num, int):
        if num >= 100000000:
            return f"{num/100000000:.1f}亿"
        elif num >= 10000:
            return f"{num/10000:.1f}万"
    return str(num)

def main():
    parser = argparse.ArgumentParser(description="Daily Trending News Fetcher")
    parser.add_argument("--source", required=True, help="Platform: bilibili, douban, github, zhihu")
    parser.add_argument("--time", default=None, help="Date (format: YYYY-MM-DD), defaults to today")
    parser.add_argument("--count", type=int, default=10, help="Number of items")
    
    args = parser.parse_args()
    
    news_time = args.time
    if news_time is None:
        news_time = datetime.now().strftime("%Y-%m-%d")
    
    location = args.source.lower()
    source_key = LOCATION_SOURCE_MAP.get(location)
    
    if not source_key:
        print(f"Error: Unknown platform '{args.source}'")
        print("Available: bilibili, bilibili(b站), douban(豆瓣), github, zhihu")
        return
    
    source_name, fetcher = SOURCE_FETCHERS.get(source_key, ("Unknown", lambda x: []))
    
    print(f"{'='*50}")
    print(f"  {source_name} - {news_time}")
    print(f"{'='*50}")
    
    items = fetcher(args.count)
    
    if "error" in items[0]:
        print(f"Error: {items[0]['error']}")
        return
    
    if not items:
        print("No data found")
        return
    
    for i, item in enumerate(items, 1):
        print(f"\n[{i:02d}] {item.get('title', '')}")
        if item.get('hot') and isinstance(item.get('hot'), int):
            print(f"    热度: {format_number(item.get('hot', 0))}")
        if item.get('rate'):
            print(f"    评分: {item.get('rate', '')}")
        if item.get('desc'):
            print(f"    {item.get('desc', '')[:60]}")

if __name__ == "__main__":
    main()
