#!/usr/bin/env python3
"""
提取 report.json 中的 `summary` 和 `final_matches` 字段，只保留这两项并写入输出文件。
默认读取 `library/report.json`，并写入 `library/report_summary.json`。

用法：
  python3 extract_report.py                 # 使用默认路径
  python3 extract_report.py -i path/to/report.json -o out.json
  python3 extract_report.py --keep summary   # 只保留 summary
  python3 extract_report.py --keep final_matches # 只保留 final_matches

脚本不会修改原文件，也不会提交 git。
"""
from pathlib import Path
import argparse
import json
import sys


def parse_args():
    p = argparse.ArgumentParser(description="Extract summary / final_matches from report.json")
    p.add_argument("-i", "--input", type=Path, default=Path("library/report.json"),
                   help="input JSON file (default: library/report.json)")
    p.add_argument("-o", "--output", type=Path, default=Path("library/report_summary.json"),
                   help="output JSON file (default: library/report_summary.json)")
    p.add_argument("--keep", choices=("summary", "final_matches", "both"), default="both",
                   help="which fields to keep (default: both)")
    return p.parse_args()


def main():
    args = parse_args()

    if not args.input.exists():
        print(f"Error: input file not found: {args.input}", file=sys.stderr)
        sys.exit(2)

    try:
        data = json.loads(args.input.read_text(encoding="utf-8"))
    except Exception as e:
        print(f"Error reading/parsing {args.input}: {e}", file=sys.stderr)
        sys.exit(1)

    out = {}
    if args.keep in ("summary", "both"):
        if "summary" in data:
            out["summary"] = data["summary"]
        else:
            print("Warning: 'summary' not found in input", file=sys.stderr)
    if args.keep in ("final_matches", "both"):
        if "final_matches" in data:
            out["final_matches"] = data["final_matches"]
        else:
            print("Warning: 'final_matches' not found in input", file=sys.stderr)

    try:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(out, ensure_ascii=False, indent=2), encoding="utf-8")
        print(f"Wrote {args.output}")
    except Exception as e:
        print(f"Error writing {args.output}: {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
