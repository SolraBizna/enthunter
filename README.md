Did you know that on Linux it's possible to encrypt a disk partition in-place using plain ole' dm-crypt?

Did you know that it's dangerous and foolhardy to do so without a backup, since a system failure in the middle of the process leaves the partition half-encrypted with no way to determine where the process stopped?

Did you do it anyway?

If you answered "no" to the second question or "yes" to the third, chances are you have found this repository after doing a Google search like "cryptsetup plain in-place abort" or "dm-crypt in-place crashed partway". I have good news and bad news for you.

The good news is that you are looking at the README for a tool designed specifically to guess where your foolish encryption process was when it failed.

The bad news is that there are lots of circumstances where this tool will not give a perfect answer, including at least:

- The process stopped due to a power failure, stopping the hard disk from flushing its buffers, leaving chaos and destruction in its wake. Expect corruption on the order of your hard disk's buffer size.
- The process stopped while encrypting highly-compressed or already-otherwise-encrypted data. Expect corruption on this file, and possibly some similar files unlucky enough to be stored after it. (Fortunately, since file-system *data structures* are relatively rare and moderately compressible, they will probably not run afoul of this problem.)
- You made a mistake during your original attempt, such as an incorrect passphrase. If the mistake is severe enough, your data is probably unrecoverable.

Meanwhile, let's think positively and hope that at least some of your data is recoverable!

(Don't run **any** example commands until you're certain you understand every part of them! `man 8 cryptsetup` and `man 1 dd` are your friend. **Especially don't forget to substitute your own device, cipher, hash/key, byte number, etc.!**)

**Step 1:** Ensure you have the Rust compiler and Cargo build system installed.  If you don't, [here are simple, fast directions for installing them.](https://www.rust-lang.org/en-US/install.html)

**Step 2:** Get yourself a build of `enthunter`.

```sh
$ git clone https://github.com/SolraBizna/enthunter
Cloning into 'enthunter'...
(...snip...)
$ cd enthunter
$ cargo build --release
(...snip...)
    Finished release [optimized] target(s) in 12.34 secs
```

**Step 3:** Map your encrypted drive. Example using `cryptsetup`:

```sh
$ sudo cryptsetup open --type plain -c aes-cbc-essiv:sha256 -h ripemd160 -s 256 /dev/sdc3 patient
```

**Step 4:** Use `enthunter` to locate the first almost-certainly-not-encrypted sector of the partition.

```sh
$ sudo ~/enthunter/target/release/enthunter /dev/sdc3 /dev/mapper/patient
Our best guess is byte number 123456789123.
```

If you have `pv` installed, you can use it to get a slick progress bar (which will end up half-merged with the byte number output, so be careful):

```sh
$ sudo pv /dev/sdc3 | sudo ~/enthunter/target/release/enthunter /dev/stdin /dev/mapper/patient
```

If you're unlucky, you'll get output like this:

```
At byte number 975318642, BOTH sides appear unencrypted.
```

This is unfortunate, and likely indicates a large amount of corruption. (Or that you mapped the partition incorrectly.)

If you're REALLY unlucky, you'll get this dreadful message:

```
It looks like the left side is fully encrypted.
```

Which might be good news, indicating that the encryption process succeeded after all... or bad news, indicating that all the data is corrupted beyond recovery. Either way, there's nothing more you can do but try to `fsck` and/or `mount` the drive and hope for the best.

**Step 5:** Resume the in-place encryption process at the byte number in question.

```
$ sudo dd if=/dev/sdc3 of=/dev/mapper/patient bs=1048576 iflag=skip_bytes oflag=seek_bytes status=progress skip=123456789123 seek=123456789123
```

**Step 6:** Cleanup!

`fsck`. `mount`. Perform whatever checks are possible. It's overwhelmingly likely that at least *some* of your data was corrupted.

For example, if the patient was a backup drive in an `rsync`-based backup system like [Slugger](https://github.com/SolraBizna/slugger), you could do an otherwise normal backup with the `-c` option, which will check the integrity even of files whose size and metadata are already correct.

# Technical Details

`enthunter` works by reading each sector of the partition, both on the raw and mapped sides. It measures the Shannon entropy of each sector. If the Shannon entropy is below 3700 bits per 512 bytes, it considers the sector to be "almost certainly unencrypted". Encryption should, theoretically, be able to result in any particular bitstream... but in practice, encrypted bitstreams with low Shannon entropy are spectacularly unlikely. (Run `enthunter` with only one parameter to get a quick analysis of the Shannon entropy of a file/device... but bear in mind that it will consume 4 bytes of memory for every 512 bytes of data, so maybe don't run it on your multi-terabyte backup drive.)

`enthunter` will give output the first time it encounters a raw sector that appears "almost certainly unencrypted". It expects the mapped sector to be in the opposite state—reading the mapped sector when the raw sector is not encrypted should result in high-entropy garbage. If this is the case, it gives the "best guess" message. If both sectors appear "almost certainly unencrypted", it will express its doubts and give up. If it never encounters a raw sector that appears "almost certainly unencrypted", then... it will give up, job done as well as it can.

If that sounds hacky, somewhat arbitrary, and a bit fragile, it's because it is. You're far better off encrypting a drive the proper way in the first place; backup, wipe, encrypt, restore. At any step of that process (barring backup failure) you can have all the system failures you like without losing all your data. Of course, sometimes you're too poor to afford extra storage/bandwidth, you know the risks, you know the odds of successful recovery, and you decide to go ahead and do it... and accept the consequences.
