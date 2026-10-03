#!/usr/bin/env python3
"""Read-only protected-environment gate using the existing Actions token."""
from __future__ import annotations
import argparse
import json
import os
import re
import sys
import urllib.error
import urllib.request

class Blocked(ValueError): pass

def check(environment):
    rules=environment.get('protection_rules',[])
    if not any(p.get('type') == 'required_reviewers' and p.get('reviewers') for p in rules):
        raise Blocked('production-signing required-reviewer protection is missing')
    policy=environment.get('deployment_branch_policy') or {}
    if not (policy.get('protected_branches') or policy.get('custom_branch_policies')):
        raise Blocked('production-signing deployment branch/tag policy is missing')

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--repository',required=True)
    args=parser.parse_args()
    try:
        if not re.fullmatch(r'[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+',args.repository): raise Blocked('invalid repository')
        token=os.environ.get('GITHUB_TOKEN','')
        if not token: raise Blocked('existing Actions read token is missing')
        request=urllib.request.Request('https://api.github.com/repos/' + args.repository + '/environments/production-signing',
            headers={'Authorization':'Bearer ' + token,'Accept':'application/vnd.github+json','X-GitHub-Api-Version':'2022-11-28'})
        with urllib.request.urlopen(request, timeout=30) as response: environment=json.load(response)
        check(environment)
        print('SIGNING_PROTECTION_PASS (runner labels/certificate checked by signing job)');return 0
    except (Blocked,urllib.error.URLError,KeyError,ValueError) as error:
        print('SIGNING_BLOCKED: ' + str(error),file=sys.stderr);return 1

if __name__ == '__main__': sys.exit(main())
