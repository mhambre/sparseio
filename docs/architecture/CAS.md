# Content-Addressable Storage (CAS)

In order to optimize our cache storage efficiency we utilize a content-addressable storage (CAS) system.
Large storage services take advantage of this for files that may be duplicated with only partial differences.
This can be especially useful for AI/ML workloads and is exactly how HuggingFace stores their models and
datasets through their XET architecture. It also has its uses in storage of database backups, ISOs, and more.

Below you can see a simple example of how CAS deduplicates data. In this example we have two documents that are
mostly the same except for the middle paragraph. Rather than caching 6 chunks of data total (3 for each document),
we are able to dedupe the first and last chunk, resulting in only 4 chunks of data being stored in our cache. In
something like a SFT (Supervised Finetuned) model, this can be a huge space saver as large amounts of tensors may
remain unchanged, or for full database backup where only a few records may have changed since the last backup.

<img src="../static/sparseio-cas-split-diagram.png" alt="CAS Example" width="1200"/>
