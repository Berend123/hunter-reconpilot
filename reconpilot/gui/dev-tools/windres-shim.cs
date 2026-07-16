using System;
using System.IO;
using System.Text;

internal static class Program
{
    private static int Main(string[] args)
    {
        string outFile = null;

        for (int i = 0; i < args.Length; i++)
        {
            if (string.Equals(args[i], "--output", StringComparison.OrdinalIgnoreCase) && i + 1 < args.Length)
            {
                outFile = args[i + 1];
                i++;
            }
        }

        if (string.IsNullOrWhiteSpace(outFile))
        {
            Console.Error.WriteLine("windres shim: missing --output path");
            return 1;
        }

        var fullPath = Path.GetFullPath(outFile);
        var directory = Path.GetDirectoryName(fullPath);
        if (!string.IsNullOrWhiteSpace(directory))
        {
            Directory.CreateDirectory(directory);
        }

        File.WriteAllBytes(fullPath, Encoding.ASCII.GetBytes("!<arch>\n"));
        return 0;
    }
}
