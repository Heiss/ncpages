#!/bin/sh
# Proves post_publish ran, and against which release.
echo "post_publish trigger=$NCPAGES_TRIGGER release=$NCPAGES_RELEASE_DIR" >> /work/post_publish.log
