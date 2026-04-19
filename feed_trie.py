#!/usr/bin/env python3
"""
Feed documents into trie-memory MCP server via SSE HTTP endpoint.

Usage:
  1. Start trie-memory with: cargo run --release -- --sse
  2. Run this script: python feed_trie.py
  3. Ctrl+C the server when done (triggers auto-save)

The script reads markdown files from the docs/ folder and the novel,
splits them into chunks, and sends trie_remember calls for each chunk.
"""

import json
import re
import sys
import time
import requests
import sseclient  # pip install sseclient-py

SERVER = "http://localhost:3001"
DOCS_DIR = r"W:\workspace\trie-memory\docs"
NOVEL_PATH = r"W:\workspace\bckup\..\trie-memory\..\..\FULL_DRAFT_PARTS_1-4.md"  # adjust if needed

# Adjust these paths to your actual file locations
DOC_FILES = [
    (f"{DOCS_DIR}/theory.md", "tick-frame theory"),
    (f"{DOCS_DIR}/design_radiation_binding.md", "radiation and binding design"),
    (f"{DOCS_DIR}/design_content_store.md", "content store design"),
    (f"{DOCS_DIR}/design_fibonacci_context.md", "fibonacci context design"),
    (f"{DOCS_DIR}/design_intermediate_insertion.md", "intermediate insertion design"),
    (f"{DOCS_DIR}/decisions.md", "architecture decisions"),
    (f"{DOCS_DIR}/session_2026_04_08.md", "session notes"),
]


def get_session_id():
    """Connect to SSE endpoint and get session ID from the endpoint event."""
    resp = requests.get(f"{SERVER}/sse", stream=True)
    client = sseclient.SSEClient(resp)
    for event in client.events():
        if event.event == "endpoint":
            # Extract sessionId from the endpoint URL
            data = event.data
            match = re.search(r'sessionId=([^&\s]+)', data)
            if match:
                session_id = match.group(1)
                print(f"Got session: {session_id}")
                resp.close()
                return session_id
    raise RuntimeError("Could not get session ID from SSE endpoint")


def call_tool(session_id, tool_name, arguments, req_id=1):
    """Send a tools/call JSON-RPC request to the MCP server."""
    msg = {
        "jsonrpc": "2.0",
        "id": req_id,
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": arguments
        }
    }
    resp = requests.post(
        f"{SERVER}/messages?sessionId={session_id}",
        json=msg,
        headers={"Content-Type": "application/json"}
    )
    if resp.status_code != 200:
        print(f"  ERROR: HTTP {resp.status_code}: {resp.text[:200]}")
        return None
    return resp.json() if resp.text else None


def chunk_text(text, max_words=150):
    """Split text into chunks at paragraph boundaries, respecting max word count."""
    paragraphs = re.split(r'\n\s*\n', text)
    chunks = []
    current = []
    current_words = 0

    for para in paragraphs:
        para = para.strip()
        if not para:
            continue
        # Skip headers-only or very short lines
        words = len(para.split())
        if current_words + words > max_words and current:
            chunks.append('\n\n'.join(current))
            current = [para]
            current_words = words
        else:
            current.append(para)
            current_words += words

    if current:
        chunks.append('\n\n'.join(current))

    return chunks


def extract_topic_from_header(text):
    """Try to find a markdown header to use as topic."""
    match = re.search(r'^#+ (.+)', text, re.MULTILINE)
    if match:
        return match.group(1).strip()
    return None


def feed_doc_file(session_id, filepath, base_topic, req_counter):
    """Read a doc file and feed it as trie_remember chunks."""
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            text = f.read()
    except FileNotFoundError:
        print(f"  SKIP: {filepath} not found")
        return req_counter

    # Split by headers first for topic labeling
    sections = re.split(r'(?=^## )', text, flags=re.MULTILINE)

    for section in sections:
        section = section.strip()
        if not section:
            continue

        topic = extract_topic_from_header(section) or base_topic
        chunks = chunk_text(section, max_words=150)

        for chunk in chunks:
            chunk = chunk.strip()
            if len(chunk.split()) < 5:  # skip tiny chunks
                continue

            req_counter += 1
            result = call_tool(session_id, "trie_remember", {
                "content": chunk,
                "topic": topic
            }, req_counter)

            if result:
                print(f"  [{req_counter}] Stored {len(chunk.split())}w | topic: {topic[:50]}")
            else:
                print(f"  [{req_counter}] FAILED | topic: {topic[:50]}")

            time.sleep(0.05)  # small delay to not overwhelm

    return req_counter


def feed_novel_chapters(session_id, filepath, req_counter, max_chapters=5):
    """Feed novel chapters as summaries (first paragraph of each chapter)."""
    try:
        with open(filepath, 'r', encoding='utf-8') as f:
            text = f.read()
    except FileNotFoundError:
        print(f"  SKIP: {filepath} not found")
        return req_counter

    # Split by chapter headers
    chapters = re.split(r'(?=^# )', text, flags=re.MULTILINE)

    for chapter in chapters[:max_chapters]:
        chapter = chapter.strip()
        if not chapter:
            continue

        # Get chapter title
        title_match = re.match(r'^# (.+)', chapter)
        title = title_match.group(1).strip() if title_match else "unknown chapter"

        # Feed first few paragraphs (the opening)
        paragraphs = [p.strip() for p in chapter.split('\n\n') if p.strip() and not p.strip().startswith('#') and not p.strip().startswith('**Distance')]
        opening = ' '.join(paragraphs[:3])[:800]  # first 3 paragraphs, max 800 chars

        if len(opening.split()) < 10:
            continue

        req_counter += 1
        result = call_tool(session_id, "trie_remember", {
            "content": opening,
            "topic": f"novel: {title}"
        }, req_counter)

        if result:
            print(f"  [{req_counter}] Chapter: {title[:40]} | {len(opening.split())}w")
        else:
            print(f"  [{req_counter}] FAILED: {title[:40]}")

        time.sleep(0.05)

    return req_counter


def main():
    print("=" * 60)
    print("trie-memory document feeder")
    print("=" * 60)
    print(f"Server: {SERVER}")
    print()

    # Test connection
    try:
        resp = requests.get(f"{SERVER}/sse", stream=True, timeout=3)
    except requests.ConnectionError:
        print("ERROR: Cannot connect to trie-memory server.")
        print("Start it with: cargo run --release -- --sse")
        sys.exit(1)

    print("Connecting to SSE endpoint...")
    session_id = get_session_id()
    print(f"Session: {session_id}")
    print()

    req_counter = 0

    # Feed doc files
    print("--- Feeding theory docs ---")
    for filepath, topic in DOC_FILES:
        print(f"\nFile: {filepath}")
        req_counter = feed_doc_file(session_id, filepath, topic, req_counter)

    # Feed novel (first few chapters)
    # Uncomment if you want to include novel excerpts:
    # print("\n--- Feeding novel chapters ---")
    # novel_path = r"W:\workspace\trie-memory\FULL_DRAFT_PARTS_1-4.md"  # adjust path
    # req_counter = feed_novel_chapters(session_id, novel_path, req_counter, max_chapters=10)

    # Snapshot
    print("\n--- Saving snapshot ---")
    req_counter += 1
    result = call_tool(session_id, "trie_snapshot", {
        "path": "W:\\workspace\\trie-memory\\trie-memory.dat",
        "word_path": "W:\\workspace\\trie-memory\\word-trie-memory.dat",
        "content_path": "W:\\workspace\\trie-memory\\content-store.json"
    }, req_counter)
    print(f"Snapshot: {result}")

    # Also save concepts
    req_counter += 1
    result = call_tool(session_id, "concept_snapshot", {
        "path": "W:\\workspace\\trie-memory\\concept-store.json"
    }, req_counter)
    print(f"Concept snapshot: {result}")

    print(f"\nDone! Fed {req_counter} requests.")
    print("You can also Ctrl+C the server for auto-save.")


if __name__ == "__main__":
    main()
