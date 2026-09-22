// Used by PowerShell 5.1/7 via Add-Type. No third-party tools or diskpart needed.
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using Microsoft.Win32.SafeHandles;

public static class UsbImageWriter
{
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern SafeFileHandle CreateFile(string path, uint access, uint share,
        IntPtr security, uint creation, uint flags, IntPtr template);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool DeviceIoControl(SafeFileHandle handle, uint code, IntPtr input,
        uint inputSize, byte[] output, uint outputSize, out uint returned, IntPtr overlapped);

    static SafeFileHandle Open(string path, uint access)
    {
        var handle = CreateFile(path, access, 3, IntPtr.Zero, 3, 0x80000000, IntPtr.Zero);
        if (handle.IsInvalid) {
            int error = Marshal.GetLastWin32Error();
            handle.Dispose();
            throw new Win32Exception(error, "Cannot open " + path);
        }
        return handle;
    }

    static byte[] Control(SafeFileHandle handle, uint code, int bytes)
    {
        byte[] result = new byte[bytes];
        uint returned;
        if (!DeviceIoControl(handle, code, IntPtr.Zero, 0, result, (uint)bytes, out returned, IntPtr.Zero))
            throw new Win32Exception(Marshal.GetLastWin32Error(), "DeviceIoControl " + code.ToString("X"));
        if (returned < bytes) throw new IOException("Incomplete device information.");
        return result;
    }

    static int DeviceNumber(SafeFileHandle handle)
    {
        byte[] info = Control(handle, 0x2D1080, 12); // IOCTL_STORAGE_GET_DEVICE_NUMBER
        if (BitConverter.ToUInt32(info, 0) != 7) throw new IOException("Not a disk device.");
        return BitConverter.ToInt32(info, 4);
    }

    static void ReadExactly(Stream stream, byte[] buffer, int count)
    {
        int total = 0;
        while (total < count) {
            int got = stream.Read(buffer, total, count - total);
            if (got == 0) throw new EndOfStreamException("Incomplete image/device read.");
            total += got;
        }
    }

    public static string ImageHash(Stream source)
    {
        if (source.Length < 1024 || source.Length % 512 != 0)
            throw new IOException("Expected a sector-aligned RAW image.");
        source.Position = 0;
        byte[] sector = new byte[512];
        ReadExactly(source, sector, sector.Length);
        long start = BitConverter.ToUInt32(sector, 454);
        long count = BitConverter.ToUInt32(sector, 458);
        if (sector[510] != 0x55 || sector[511] != 0xAA || sector[450] != 0xEF ||
            start == 0 || count == 0 || (start + count) * 512 > source.Length)
            throw new IOException("Expected a MIND CORE RAW image with an MBR/UEFI partition.");
        source.Position = start * 512;
        ReadExactly(source, sector, sector.Length);
        if (sector[510] != 0x55 || sector[511] != 0xAA ||
            System.Text.Encoding.ASCII.GetString(sector, 43, 11) != "MIND CORE  " ||
            System.Text.Encoding.ASCII.GetString(sector, 54, 8) != "FAT16   ")
            throw new IOException("MIND CORE FAT16 partition not found.");
        source.Position = 0;
        using (var sha = SHA256.Create()) {
            string hash = BitConverter.ToString(sha.ComputeHash(source)).Replace("-", "").ToLowerInvariant();
            source.Position = 0;
            return hash;
        }
    }

    // Stream core is tested on temporary files, never on a physical disk.
    public static void CopyAndVerify(Stream source, Stream target, long length, long diskSize,
        string expectedHash, Action flush, Action<int> progress)
    {
        if (length <= 0 || length % 512 != 0 || diskSize < length || diskSize % 512 != 0)
            throw new IOException("Invalid image/device size.");
        byte[] buffer = new byte[4 * 1024 * 1024];
        source.Position = 0;
        target.Position = 0;
        long copied = 0;
        while (copied < length) {
            int count = (int)Math.Min(buffer.Length, length - copied);
            ReadExactly(source, buffer, count);
            target.Write(buffer, 0, count);
            copied += count;
            progress((int)(copied * 100 / length));
        }
        // Clear stale backup GPT outside the image, retaining exact image bytes.
        long tail = Math.Max(length, diskSize - 33 * 512);
        target.Position = tail;
        byte[] zeros = new byte[(int)(diskSize - tail)];
        target.Write(zeros, 0, zeros.Length);
        flush();
        target.Position = 0;
        using (var sha = SHA256.Create()) {
            long remaining = length;
            while (remaining > 0) {
                int count = (int)Math.Min(buffer.Length, remaining);
                ReadExactly(target, buffer, count);
                sha.TransformBlock(buffer, 0, count, buffer, 0);
                remaining -= count;
            }
            sha.TransformFinalBlock(new byte[0], 0, 0);
            string actual = BitConverter.ToString(sha.Hash).Replace("-", "").ToLowerInvariant();
            if (!String.Equals(actual, expectedHash, StringComparison.OrdinalIgnoreCase))
                throw new IOException("SHA256 mismatch. The USB drive is NOT ready.");
        }
    }

    public static void WriteDisk(Stream image, int number, long expectedSize, string expectedHash,
        string[] volumes, Action<int> progress)
    {
        var locks = new List<SafeFileHandle>();
        try {
            // Hold every volume lock until write and verification have finished.
            // Lock fails for open files, pagefiles or system volumes; never force it.
            foreach (string volume in volumes) {
                var handle = Open(volume.TrimEnd('\\'), 0xC0000000);
                locks.Add(handle);
                if (DeviceNumber(handle) != number) throw new IOException("Volume belongs to another disk.");
                Control(handle, 0x90018, 0); // FSCTL_LOCK_VOLUME
            }
            foreach (var handle in locks) Control(handle, 0x90020, 0); // FSCTL_DISMOUNT_VOLUME
            using (var handle = Open(@"\\.\PhysicalDrive" + number, 0xC0000000)) {
                if (DeviceNumber(handle) != number ||
                    BitConverter.ToInt64(Control(handle, 0x7405C, 8), 0) != expectedSize)
                    throw new IOException("Device changed after selection.");
                using (var disk = new FileStream(handle, FileAccess.ReadWrite, 4096, false)) {
                    CopyAndVerify(image, disk, image.Length, expectedSize, expectedHash,
                        delegate { disk.Flush(true); }, progress);
                }
            }
        }
        finally {
            foreach (var handle in locks) handle.Dispose(); // releases the locks on failure too
        }
    }
}
