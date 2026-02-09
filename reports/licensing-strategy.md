# Licensing Strategy

## Current Setup: Dual License (Proprietary Noncommercial + Commercial)

### Private Repository (Source Code)
- **License**: Proprietary / All Rights Reserved
- **Access**: Restricted to authorized developers only
- **Rights**: Full control over source code
- **Distribution**: Source code is NOT distributed

### Public Repository (Binaries + Documentation)
- **Primary License**: Proprietary Noncommercial (Free for personal use only)
- **Secondary License**: Commercial License (Paid - required for ALL business/organizational use)
- **Access**: Public
- **Distribution**: Pre-compiled binaries and documentation
- **Rights**: Free ONLY for standalone individual personal use, paid for ALL commercial/organizational use

## What This Means

### For You (Developer)
✅ **Full control** over source code
✅ **No obligation** to share source with users
✅ **Patent protection** retained (source not published)
✅ **Can change licensing** in the future
✅ **Revenue generation** from all commercial/organizational users
✅ **Maximum protection** from unauthorized commercial use

### For Users

**Free Personal Users (Standalone Individuals):**
✅ **Free to use** the binaries for personal projects
✅ **Free to distribute** the binaries
✅ **Can modify** for personal use (if possible)
✅ **No warranty liability** (you're protected)
❌ **Cannot use for work** or professional purposes
❌ **Cannot modify source** code (not available)

**Commercial Users (ALL Businesses/Organizations):**
✅ **Must purchase license** before use
✅ **No trial period** available
✅ **Professional support** included
✅ **Unrestricted work use** after purchase
❌ **Cannot use for free** (even for evaluation)

## License Text Placement

### Private Repo
Add to root:
```
# All Rights Reserved

Copyright (c) 2024 Thomas Delalonde

This software and associated documentation files (the "Software") are
proprietary and confidential. Unauthorized copying, distribution, or
use of this Software, via any medium, is strictly prohibited.
```

### Public Repo
Add to root:
```
MIT License (see LICENSE file)
```

Already created: `LICENSE` file in this repo (will be copied to public repo)

## Alternative Licensing Options

### If You Want to Keep Source Private but Viewable

**1. Business Source License (BSL)**
- Source code visible on GitHub
- Free for non-commercial use
- Commercial use requires license after threshold
- Converts to open source after time period (e.g., 2 years)
- **Example**: MariaDB, CockroachDB

**2. Fair Source License**
- Source code visible
- Free for up to N users (you choose N)
- Beyond N users requires paid license
- **Example**: Sentry (historically)

**3. Commons Clause**
- Open source + restriction on selling the software as a service
- Can view and modify source
- Cannot resell as SaaS
- **Example**: Redis (some modules)

### If You Want Full Open Source

**1. MIT License** ⭐ Recommended if going open source
- Most permissive
- Easy to understand
- Maximum adoption
- **Examples**: jQuery, .NET Core, Rails

**2. Apache License 2.0**
- Like MIT but with explicit patent grant
- Better for corporate use
- Requires attribution in derived works
- **Examples**: Kubernetes, Swift, Android

**3. Mozilla Public License 2.0 (MPL)**
- File-level copyleft
- Can mix with proprietary code
- Modifications must be open sourced
- **Examples**: Firefox, Thunderbird

**4. GNU GPL v3**
- Strong copyleft
- Derivatives must be GPL
- Ensures ecosystem stays open
- **Examples**: Linux, Git, Bash

## Recommendations by Use Case

### Current Strategy: Private Source, Public Binaries
✅ **Recommended**: MIT License for public repo
- Maximizes user adoption
- Simple and well-understood
- Protects you from liability
- No conflict with private source

### Future: Want to Monetize
Consider:
- **BSL** - Free for small use, paid for enterprise
- **Dual License** - GPL for open source, Commercial for proprietary use
- **Fair Source** - Free up to user limit

### Future: Want to Go Fully Open
Consider:
- **MIT** - Maximum adoption, simple
- **Apache 2.0** - Better patent protection
- **MPL 2.0** - Protect your core while allowing extensions

### Future: Want to Ensure Derivatives Stay Open
Consider:
- **GPL v3** - Strong copyleft
- **AGPL v3** - GPL + network use (for SaaS)

## Implementation

### Current Setup (Recommended)

**Private Repo** (`devcli-private`):
```
No LICENSE file (or create PROPRIETARY-LICENSE.txt)
```

**Public Repo** (`devcli`):
```
LICENSE (MIT) ✅ Already created
```

### Update Cargo.toml

```toml
[workspace.package]
version = "0.1.0"
edition = "2025"
authors = ["Thomas Delalonde"]
license = "MIT"  # ← For the binaries distributed
```

### Update README.md

Add license badge:
```markdown
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
```

## License Compatibility

Since you're distributing binaries, you can use dependencies with these licenses:
- ✅ MIT
- ✅ Apache 2.0
- ✅ BSD (2-clause, 3-clause)
- ✅ ISC
- ⚠️ MPL 2.0 (file-level copyleft, usually OK for binaries)
- ❌ GPL (requires your code to be GPL too - avoid or get legal advice)

Check with:
```bash
cargo tree --edges normal | grep -i gpl
```

## FAQ

**Q: Can users decompile my binaries?**
A: Technically yes, but MIT license doesn't grant them rights to the source code representation. Consider:
- Rust binaries are harder to decompile than interpreted languages
- Add anti-reverse-engineering terms if needed (separate EULA)
- Most users respect the spirit of private source

**Q: Can I change the license later?**
A: Yes, for future versions. You own the copyright.
- Already distributed binaries stay under their original license
- New releases can have new licenses
- Can dual-license (GPL + Commercial)

**Q: Do I need a CLA (Contributor License Agreement)?**
A: Not with private source. Only relevant if accepting external contributions.

**Q: What about the docs?**
A: Docs in public repo are MIT licensed (same as binaries)
- Users can copy/modify docs freely
- Could use Creative Commons instead if you prefer (CC-BY-4.0)

**Q: Should I add copyright headers to files?**
A: For private repo: Your choice
For public repo docs: Not necessary with MIT, but can add:
```
// Copyright (c) 2024 Thomas Delalonde
// SPDX-License-Identifier: MIT
```

## Summary

**Current Implementation**: ✅ **Proprietary Noncommercial + Commercial Dual License**

**Why This Approach?**
- ✅ Maximum revenue potential (all business use requires license)
- ✅ Protects from unauthorized commercial exploitation
- ✅ Still allows personal adoption by individual users
- ✅ Very restrictive (no companies/orgs/governments for free)
- ✅ No trial period confusion (purchase required from day one)
- ✅ Clear terms - either personal individual or commercial license
- ✅ Compatible with keeping source private
- ✅ Protects you from liability

**Key Restrictions**:
- ❌ No free use by companies, businesses, organizations
- ❌ No free use by government, educational, non-profit entities
- ❌ No free use by consultants, freelancers for work
- ❌ No commercial trial period
- ❌ Only standalone individuals for personal use get free license

**Completed Steps**:
1. ✅ LICENSE file created (Proprietary Noncommercial)
2. ✅ LICENSE-COMMERCIAL.md created with pricing
3. ✅ Cargo.toml updated with `license = "Proprietary"`
4. ✅ README.md updated with license badges and explanation
5. ✅ Removed 32-day trial period language
6. ✅ Restricted free use to standalone individuals only

**Future Options**:
- Can relax restrictions if adoption is low
- Can add educational/non-profit discounts (currently 50% off)
- Can add limited trial period if market demands
- Can adjust pricing based on market response
- Current choice is most restrictive, can only become more permissive
