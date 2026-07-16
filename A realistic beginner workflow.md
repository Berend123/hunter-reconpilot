# 1. Read the program scope carefully

Before touching the target:

- Read in-scope assets
- Read out-of-scope rules
- Check forbidden testing methods
- Look for known exclusions:
    - rate limiting
    - DoS
    - social engineering
    - physical attacks

Common platforms:

- [HackerOne](https://www.hackerone.com/?utm_source=chatgpt.com)
- [Bugcrowd](https://www.bugcrowd.com/?utm_source=chatgpt.com)
- [Intigriti](https://www.intigriti.com/?utm_source=chatgpt.com)

A surprising number of duplicates/wasted effort come from skipping this.

---

# 2. Passive reconnaissance first

Before active testing, map the surface area.

Typical things to check:

## Main pages

Look for:

- technologies used
- app structure
- auth flows
- APIs
- admin panels
- third-party integrations

Extensions like:

- [Wappalyzer](https://www.wappalyzer.com/?utm_source=chatgpt.com)

help identify frameworks and stacks.

---

## robots.txt

Example:

```
/robots.txt
```

Can reveal:

- hidden paths
- admin panels
- staging areas
- API routes
- forgotten directories

Not always useful, but always quick to check.

---

## sitemap.xml

Example:

```
/sitemap.xml
```

Often reveals:

- old endpoints
- orphaned pages
- API docs
- parameterized URLs

---

## security.txt

Example:

```
/.well-known/security.txt
```

May contain:

- disclosure contacts
- extra scope info
- domains
- policies

---

## favicon hash / headers

Useful for identifying:

- frameworks
- CDNs
- WAFs
- panel software

Tools:

- [Shodan](https://www.shodan.io/?utm_source=chatgpt.com)
- [Censys](https://search.censys.io/?utm_source=chatgpt.com)

---

# 3. Enumerate subdomains

This is huge in bug bounty work.

Common tools:

- [subfinder](https://github.com/projectdiscovery/subfinder?utm_source=chatgpt.com)
- [amass](https://github.com/owasp-amass/amass?utm_source=chatgpt.com)
- [assetfinder](https://github.com/tomnomnom/assetfinder?utm_source=chatgpt.com)

You’re looking for:

- dev
- staging
- beta
- old apps
- forgotten APIs
- admin portals

Often the “weak” asset is not the main app.

---

# 4. Probe live hosts

Once you have domains:

Tools:

- [httpx](https://github.com/projectdiscovery/httpx?utm_source=chatgpt.com)

Check:

- titles
- status codes
- tech stack
- redirects
- screenshots

You want a quick mental map:

- marketing site
- auth service
- API gateway
- internal tooling
- mobile backend

---

# 5. Crawl the application

Now you start discovering endpoints.

Typical methods:

- Manual browsing
- Burp Proxy history
- Burp sitemap
- JS file analysis

Tools:

- [Katana](https://github.com/projectdiscovery/katana?utm_source=chatgpt.com)
- [waymore](https://github.com/xnl-h4ck3r/waymore?utm_source=chatgpt.com)

---

# 6. Check JavaScript files

This is extremely important.

Look for:

- hidden API endpoints
- secrets
- internal URLs
- GraphQL endpoints
- AWS buckets
- tokens
- feature flags

Tools:

- [LinkFinder](https://github.com/GerbenJavado/LinkFinder?utm_source=chatgpt.com)
- [SecretFinder](https://github.com/m4ll0k/SecretFinder?utm_source=chatgpt.com)

---

# 7. Archive/history digging

Old endpoints are gold.

Check:

- deleted APIs
- legacy panels
- forgotten routes

Sources:

- Wayback Machine
- [Common Crawl](https://commoncrawl.org/?utm_source=chatgpt.com)

Tools:

- gau
- waybackurls
- waymore

---

# 8. Identify auth boundaries

Before testing vulnerabilities:  
Map:

- user roles
- privilege levels
- account creation
- password reset
- invitation flows
- file uploads
- payment flows

This is where many high-value bugs appear:

- IDOR
- access control
- privilege escalation

---

# 9. Build a “test checklist”

Once you understand the app, start structured testing:

For each endpoint:

- auth bypass?
- IDOR?
- rate limiting?
- SSRF?
- XSS?
- CSRF?
- SQLi?
- mass assignment?
- business logic abuse?

---

# 10. Focus on unusual behavior

The best bounty findings often come from:

- edge cases
- workflow abuse
- race conditions
- broken assumptions
- inconsistent authorization

Not just automated scanning.

---

# A realistic beginner workflow

A clean beginner flow might be:

1. Open target in Burp browser
2. Check:
    - robots.txt
    - sitemap.xml
    - security.txt
3. Browse manually
4. Observe requests in Burp
5. Identify APIs
6. Enumerate parameters
7. Test access control
8. Analyze JS files
9. Check archived URLs
10. Start focused testing

---

# One important mindset shift

Don’t think:

> “How do I find XSS?”

Think:

> “How does this application actually work?”

The better you understand:

- trust boundaries
- user flows
- data handling
- assumptions

…the more valuable bugs you’ll find.



A lot of the **surface mapping** can be automated. The actual high-value vulnerability discovery usually becomes increasingly manual.

A good mental model is:

- **Automation = breadth**
    
- **Manual testing = depth**
    

The best hunters automate repetitive discovery work so they can spend more time thinking.

---

# Usually Automated

## Subdomain enumeration

Very commonly automated.

Tools:

- [subfinder](https://github.com/projectdiscovery/subfinder?utm_source=chatgpt.com)
    
- [amass](https://github.com/owasp-amass/amass?utm_source=chatgpt.com)
    
- [assetfinder](https://github.com/tomnomnom/assetfinder?utm_source=chatgpt.com)
    

Example:

```bash
subfinder -d target.com
```

---

## Live host probing

Extremely automatable.

Tools:

- [httpx](https://github.com/projectdiscovery/httpx?utm_source=chatgpt.com)
    

You can automate:

- status codes
    
- titles
    
- screenshots
    
- tech stack detection
    
- CDN/WAF detection
    

Example:

```bash
cat subs.txt | httpx -title -tech-detect
```

---

## robots.txt / sitemap.xml discovery

Easy automation target.

You can script:

- `/robots.txt`
    
- `/sitemap.xml`
    
- `/security.txt`
    
- common admin paths
    

This is very lightweight recon.

---

## JavaScript endpoint extraction

Highly automatable.

Tools:

- [LinkFinder](https://github.com/GerbenJavado/LinkFinder?utm_source=chatgpt.com)
    
- [SecretFinder](https://github.com/m4ll0k/SecretFinder?utm_source=chatgpt.com)
    

Automate:

- endpoint extraction
    
- secrets
    
- tokens
    
- internal URLs
    

---

## URL collection from archives

Almost entirely automated.

Tools:

- gau
    
- waymore
    
- waybackurls
    

Example:

```bash
gau target.com
```

---

## Crawling

Automatable to a point.

Tools:

- [Katana](https://github.com/projectdiscovery/katana?utm_source=chatgpt.com)
    
- [hakrawler](https://github.com/hakluke/hakrawler?utm_source=chatgpt.com)
    

Automate:

- endpoint discovery
    
- parameter discovery
    
- forms
    
- JS crawling
    

---

## Screenshotting assets

Very useful at scale.

Tools:

- aquatone
    
- gowitness
    

Lets you visually scan:

- login portals
    
- Jenkins
    
- Grafana
    
- old apps
    
- admin dashboards
    

---

## Basic vulnerability pattern matching

Partially automatable.

Tools:

- [Nuclei](https://github.com/projectdiscovery/nuclei?utm_source=chatgpt.com)
    

Good for:

- exposed panels
    
- known CVEs
    
- misconfigurations
    
- default creds
    
- dangerous headers
    

But:

- huge false positive risk
    
- lots of duplicates
    
- noisy if misused
    

---

# Usually Semi-Automated

## Parameter discovery

Tools help, but manual review matters.

Tools:

- arjun
    
- ParamSpider
    

These help find:

- hidden GET params
    
- API parameters
    

But humans still interpret impact.

---

## Content discovery

Tools:

- ffuf
    
- feroxbuster
    
- dirsearch
    

Automate:

- hidden directories
    
- backup files
    
- admin paths
    

But you still manually investigate findings.

---

# Mostly Manual

This is where real bounty money often lives.

---

## Access control / IDOR

Hard to automate properly.

Requires understanding:

- roles
    
- workflows
    
- ownership logic
    

Example:

- changing account IDs
    
- testing org boundaries
    
- abusing APIs
    

Humans dominate here.

---

## Business logic flaws

Mostly manual.

Examples:

- coupon abuse
    
- payment bypass
    
- race conditions
    
- invite abuse
    
- workflow manipulation
    

Automation struggles because it lacks context.

---

## Complex XSS

Basic reflection can be automated.

Real exploitable XSS often requires:

- context analysis
    
- sanitizer bypasses
    
- framework understanding
    

---

## Race conditions

Can be assisted by tooling:

- Turbo Intruder
    
- race scripts
    

But identifying the vulnerable workflow is usually manual.

---

# A very common modern workflow

People increasingly build pipelines like:

```bash
subfinder -> httpx -> katana -> nuclei
```

or:

```bash
subfinder
  ↓
httpx
  ↓
gau / waybackurls
  ↓
katana
  ↓
nuclei
```

This creates:

- subdomains
    
- live hosts
    
- endpoints
    
- historical URLs
    
- known issue detection
    

Then the hunter manually investigates interesting assets.

---

# Important reality check

Beginners often over-automate too early.

Running:

```bash
nuclei -u target.com
```

against 500 targets usually produces:

- duplicates
    
- informational junk
    
- low-value findings
    

The highest payouts usually come from:

- understanding the app deeply
    
- noticing weird assumptions
    
- chaining small issues
    

---

# A good balance

A strong setup is:

## Automated:

- recon
    
- crawling
    
- endpoint collection
    
- screenshots
    
- passive intel
    

## Manual:

- auth testing
    
- logic abuse
    
- workflow analysis
    
- privilege escalation
    
- chaining bugs
    

That’s where bug bounty starts becoming more like investigation than scanning.