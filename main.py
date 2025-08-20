import sqlite3
import os
import platform
import shutil
from urllib.parse import urlparse
import collections
import datetime
import re
import argparse
import sys
import logging
import time
import threading
from concurrent.futures import ThreadPoolExecutor, as_completed

def setup_logging(verbose=False):
    if verbose:
        # Custom formatter with timing
        class TimingFormatter(logging.Formatter):
            def formatTime(self, record, datefmt=None):
                # Get elapsed time since start
                elapsed = time.time() - self.start_time
                hours = int(elapsed // 3600)
                minutes = int((elapsed % 3600) // 60)
                seconds = int(elapsed % 60)
                milliseconds = int((elapsed % 1) * 1000)
                return f"{hours:02d}:{minutes:02d}:{seconds:02d}.{milliseconds:03d}"
            
            def __init__(self, fmt=None, datefmt=None, style='%'):
                super().__init__(fmt, datefmt, style)
                self.start_time = time.time()
        
        # Configure logging
        logging.basicConfig(
            level=logging.INFO,
            format='%(asctime)s [%(levelname)s] %(message)s',
            datefmt='%H:%M:%S.%f'
        )
        
        # Replace the default formatter with our custom one
        for handler in logging.root.handlers:
            handler.setFormatter(TimingFormatter('%(asctime)s [%(levelname)s] %(message)s'))
        
        return True
    else:
        logging.disable(logging.CRITICAL)
        return False

def load_domain_patterns(pattern_file_path=None):
    logging.info("Starting domain pattern loading")
    start_time = time.time()
    patterns = []
    errors = []
    
    default_patterns = [
        r'^.+\.(cloudfront\.net)$', r'^.+\.(amazonaws\.com)$', r'^.+\.(herokuapp\.com)$',
        r'^.+\.(netlify\.app)$', r'^.+\.(vercel\.app)$', r'^.+\.(github\.io)$',
        r'^.+\.(firebaseapp\.com)$', r'^.+\.(appspot\.com)$', r'^.+\.(azurewebsites\.net)$',
        r'^.+\.(cloudflare\.com)$', r'^.+\.(fastly\.com)$', r'^.+\.(cdn\.com)$',
        r'^.+\.(cdn\.net)$', r'^.+\.(cdn\.org)$', r'^.+\.(s3\.amazonaws\.com)$',
        r'^.+\.(s3-website-[^.]+\.amazonaws\.com)$', r'^.+\.(elasticbeanstalk\.com)$',
        r'^.+\.(railway\.app)$', r'^.+\.(render\.com)$', r'^.+\.(fly\.io)$',
        r'^.+\.(digitaloceanspaces\.com)$', r'^.+\.(bunnycdn\.com)$',
        r'^.+\.(stackpathcdn\.com)$', r'^.+\.(keycdn\.com)$',
    ]
    
    if pattern_file_path is not None:
        logging.info(f"Loading patterns from specified file: {pattern_file_path}")
        # Pattern file explicitly specified - strict behavior
        if not os.path.exists(pattern_file_path):
            raise FileNotFoundError(f"Pattern file not found: {pattern_file_path}")
        
        try:
            with open(pattern_file_path, 'r', encoding='utf-8') as f:
                for line_num, line in enumerate(f, 1):
                    line = line.strip()
                    if line and not line.startswith('#'):
                        patterns.append((line, line_num))
            logging.info(f"Loaded {len(patterns)} patterns from {pattern_file_path}")
        except Exception as e:
            raise Exception(f"Could not load patterns from {pattern_file_path}: {e}")
        
        # Process all patterns and collect errors
        logging.info("Compiling regex patterns")
        compiled_patterns = []
        for pattern, line_num in patterns:
            try:
                compiled_patterns.append(re.compile(pattern))
            except re.error as e:
                errors.append(f"{pattern_file_path}:{line_num}: invalid regex pattern: {e}")
        
        # If any errors occurred, print them all and stop
        if errors:
            for error in errors:
                print(error)
            raise Exception("Invalid regex patterns found. Please fix the errors above.")
        
        pattern_time = (time.time() - start_time) * 1000
        logging.info(f"Successfully compiled {len(compiled_patterns)} patterns in {pattern_time:.1f}ms")
        return compiled_patterns
    
    else:
        # No pattern file specified - try domain_patterns.txt first
        default_file = 'domain_patterns.txt'
        if os.path.exists(default_file):
            logging.info(f"Loading patterns from default file: {default_file}")
            # Use domain_patterns.txt as default, but be lenient with errors
            try:
                with open(default_file, 'r', encoding='utf-8') as f:
                    for line_num, line in enumerate(f, 1):
                        line = line.strip()
                        if line and not line.startswith('#'):
                            patterns.append((line, line_num))
                logging.info(f"Loaded {len(patterns)} patterns from {default_file}")
            except Exception as e:
                print(f"Warning: Could not load patterns from {default_file}: {e}")
        
        # Process patterns from file (if any) and collect errors
        logging.info("Compiling regex patterns")
        compiled_patterns = []
        for pattern, line_num in patterns:
            try:
                compiled_patterns.append(re.compile(pattern))
            except re.error as e:
                errors.append(f"{default_file}:{line_num}: invalid regex pattern: {e}")
        
        # If we have no valid patterns from file, use default patterns
        if not compiled_patterns:
            if errors:
                print("Warning: All patterns in domain_patterns.txt failed to compile. Using default patterns.")
            logging.info("Using default patterns")
            # Use default patterns
            for i, pattern in enumerate(default_patterns, 1):
                try:
                    compiled_patterns.append(re.compile(pattern))
                except re.error as e:
                    errors.append(f"default_patterns.txt:{i}: invalid regex pattern: {e}")
            
            # If even default patterns fail, then stop
            if not compiled_patterns:
                for error in errors:
                    print(error)
                raise Exception("All available patterns failed to compile. Please fix the errors above.")
        else:
            # We have some valid patterns from file, just warn about the bad ones
            if errors:
                print("Warning: Some patterns in domain_patterns.txt failed to compile:")
                for error in errors:
                    print(error)
        
        pattern_time = (time.time() - start_time) * 1000
        logging.info(f"Successfully compiled {len(compiled_patterns)} patterns in {pattern_time:.1f}ms")
        return compiled_patterns

def apply_pattern_normalization(domain, patterns):
    # Early exit for common cases
    if not patterns:
        return domain
    
    # Try to match patterns - most domains won't match any pattern
    for pattern in patterns:
        match = pattern.match(domain)
        if match and match.groups():
            return match.group(1)
    return domain

def normalize_domain(domain, patterns=None):
    if not domain:
        return domain
    
    # Quick check for domains with 3 or fewer parts
    dot_count = domain.count('.')
    if dot_count <= 2:
        normalized_domain = domain
    else:
        # Only split if we need to truncate
        parts = domain.split('.')
        normalized_domain = '.'.join(parts[-3:])
    
    if patterns:
        normalized_domain = apply_pattern_normalization(normalized_domain, patterns)
    
    return normalized_domain

def has_valid_tld(domain):
    if not domain:
        return False
    
    # Quick check for minimum length and dots
    if len(domain) < 3 or '.' not in domain:
        return False
    
    # Find the last dot and extract TLD
    last_dot = domain.rfind('.')
    if last_dot == -1 or last_dot == len(domain) - 1:
        return False
    
    tld = domain[last_dot + 1:]
    return len(tld) >= 2 and tld.islower() and tld.isalpha()

def get_browser_history_path(browser_name='Vivaldi'):
    logging.info(f"Getting browser history path for {browser_name}")
    system = platform.system()
    
    if browser_name.lower() == 'vivaldi':
        if system == 'Windows':
            path = os.path.join(os.environ['LOCALAPPDATA'], 'Vivaldi', 'User Data', 'Default', 'History')
        elif system == 'Darwin':
            path = os.path.join(os.path.expanduser('~'), 'Library', 'Application Support', 'Vivaldi', 'Default', 'History')
        elif system == 'Linux':
            path = os.path.join(os.path.expanduser('~'), '.config', 'vivaldi', 'default', 'History')
        else:
            raise OSError(f"Unsupported browser '{browser_name}' or operating system '{system}'.")
        
        logging.info(f"Browser history path: {path}")
        return path
    
    raise OSError(f"Unsupported browser '{browser_name}' or operating system '{system}'.")

def copy_history_database(history_path, temp_path=None):
    logging.info("Copying browser history database")
    start_time = time.time()
    if temp_path is None:
        temp_path = os.path.join(os.path.expanduser("~"), "browser_history_copy.db")
    
    logging.info(f"Source: {history_path}")
    logging.info(f"Destination: {temp_path}")
    
    if not os.path.exists(history_path):
        raise FileNotFoundError(f"History file not found at {history_path}")
    
    shutil.copyfile(history_path, temp_path)
    copy_time = (time.time() - start_time) * 1000
    logging.info(f"Database copy completed in {copy_time:.1f}ms")
    return temp_path

def get_date_range(cursor):
    logging.info("Querying visit date range")
    start_time = time.time()
    try:
        cursor.execute("SELECT MIN(visit_time), MAX(visit_time) FROM visits")
        date_range = cursor.fetchone()
        earliest_timestamp, latest_timestamp = date_range
        
        chrome_epoch = datetime.datetime(1601, 1, 1)
        
        if earliest_timestamp and latest_timestamp:
            earliest_date = chrome_epoch + datetime.timedelta(microseconds=earliest_timestamp)
            latest_date = chrome_epoch + datetime.timedelta(microseconds=latest_timestamp)
            
            def format_date(date):
                day = date.day
                suffix = "th" if 4 <= day <= 20 or 24 <= day <= 30 else ["st", "nd", "rd"][day % 10 - 1]
                return date.strftime(f"%B {day}{suffix}, %Y")
            
            days_between = (latest_date - earliest_date).days
            query_time = (time.time() - start_time) * 1000
            logging.info(f"Date range: {format_date(earliest_date)} to {format_date(latest_date)} ({days_between} days) in {query_time:.1f}ms")
            return format_date(earliest_date), format_date(latest_date), days_between
        else:
            query_time = (time.time() - start_time) * 1000
            logging.warning(f"No visit data found (query took {query_time:.1f}ms)")
            return "No data available", "No data available", 0
            
    except sqlite3.OperationalError as e:
        query_time = (time.time() - start_time) * 1000
        logging.error(f"Error querying visit dates: {e} (query took {query_time:.1f}ms)")
        return "Error retrieving date", "Error retrieving date", 0

def process_url_batch(batch_urls, patterns):
    """Process a batch of URLs and return domain counts and removal count"""
    batch_domains = set()
    batch_counts = collections.Counter()
    batch_removed = 0
    
    for url_tuple in batch_urls:
        url = url_tuple[0]
        try:
            # Extract domain directly without full URL parsing overhead
            domain = urlparse(url).netloc
            if not domain:
                continue
            
            # Quick TLD check first (most common rejection)
            if not has_valid_tld(domain):
                batch_removed += 1
                continue
            
            # Normalize domain
            normalized_domain = normalize_domain(domain, patterns)
            
            # Final TLD check
            if not has_valid_tld(normalized_domain):
                batch_removed += 1
                continue
            
            # Add to results
            batch_domains.add(normalized_domain)
            batch_counts[normalized_domain] += 1
        except Exception as e:
            logging.debug(f"Could not parse URL: {url} - Error: {e}")
    
    return batch_domains, batch_counts, batch_removed

def extract_domains_from_urls(cursor, patterns=None, max_workers=None):
    logging.info("Starting domain extraction from URLs")
    start_time = time.time()
    
    try:
        cursor.execute("SELECT url FROM urls")
    except sqlite3.OperationalError as e:
        raise Exception(f"Error querying the database: {e}. The 'urls' table might not exist or the database is corrupt.")
    
    all_urls = cursor.fetchall()
    query_time = (time.time() - start_time) * 1000
    logging.info(f"Found {len(all_urls)} URLs to process (query took {query_time:.1f}ms)")
    
    # Determine optimal number of workers
    if max_workers is None:
        import multiprocessing
        max_workers = min(multiprocessing.cpu_count(), 8)  # Cap at 8 to avoid overhead
    
    # Process URLs in batches for progress logging
    batch_size = 25000  # Increased batch size for less logging overhead
    total_batches = (len(all_urls) + batch_size - 1) // batch_size
    
    logging.info(f"Using {max_workers} workers to process {total_batches} batches")
    
    unique_domains = set()
    domain_counts = collections.Counter()
    domains_removed = 0
    
    processing_start = time.time()
    
    # For small datasets or single worker, use sequential processing
    if max_workers == 1 or total_batches <= 2:
        logging.info("Using sequential processing")
        for batch_num in range(total_batches):
            batch_start = time.time()
            start_idx = batch_num * batch_size
            end_idx = min(start_idx + batch_size, len(all_urls))
            batch_urls = all_urls[start_idx:end_idx]
            
            batch_domains, batch_counts, batch_removed = process_url_batch(batch_urls, patterns)
            
            # Merge results
            unique_domains.update(batch_domains)
            domain_counts.update(batch_counts)
            domains_removed += batch_removed
            
            batch_time = (time.time() - batch_start) * 1000
            
            # Log every batch if there are 10 or fewer batches, otherwise log every 10th batch
            if total_batches <= 10 or batch_num % 10 == 0 or batch_num == total_batches - 1:
                logging.info(f"Processed batch {batch_num + 1}/{total_batches} ({end_idx}/{len(all_urls)} URLs) in {batch_time:.1f}ms")
    
    else:
        # Process batches in parallel
        logging.info("Using parallel processing")
        with ThreadPoolExecutor(max_workers=max_workers) as executor:
            # Submit all batch processing tasks
            future_to_batch = {}
            for batch_num in range(total_batches):
                start_idx = batch_num * batch_size
                end_idx = min(start_idx + batch_size, len(all_urls))
                batch_urls = all_urls[start_idx:end_idx]
                
                future = executor.submit(process_url_batch, batch_urls, patterns)
                future_to_batch[future] = batch_num
            
            # Collect results as they complete
            completed_batches = 0
            for future in as_completed(future_to_batch):
                batch_num = future_to_batch[future]
                
                try:
                    batch_domains, batch_counts, batch_removed = future.result()
                    
                    # Merge results
                    unique_domains.update(batch_domains)
                    domain_counts.update(batch_counts)
                    domains_removed += batch_removed
                    
                    completed_batches += 1
                    
                    # Log every batch if there are 10 or fewer batches, otherwise log every 10th batch
                    if total_batches <= 10 or batch_num % 10 == 0 or batch_num == total_batches - 1:
                        logging.info(f"Processed batch {batch_num + 1}/{total_batches} ({completed_batches}/{total_batches} completed)")
                    
                except Exception as e:
                    logging.error(f"Error processing batch {batch_num + 1}: {e}")

    total_processing_time = (time.time() - processing_start) * 1000
    total_time = (time.time() - start_time) * 1000
    logging.info(f"Domain extraction completed: {len(unique_domains)} unique domains, {domains_removed} removed")
    logging.info(f"Processing time: {total_processing_time:.1f}ms, Total time: {total_time:.1f}ms")
    return unique_domains, domain_counts, domains_removed

def analyze_browser_history(browser_name='Vivaldi', temp_path=None, top_domains=10, pattern_file_path=None, max_workers=None):
    logging.info("Starting browser history analysis")
    total_start_time = time.time()
    
    history_path = get_browser_history_path(browser_name)
    temp_history_path = copy_history_database(history_path, temp_path)
    patterns = load_domain_patterns(pattern_file_path)
    
    try:
        logging.info("Connecting to database")
        conn = sqlite3.connect(temp_history_path)
        cursor = conn.cursor()
        
        earliest_date, latest_date, days_between = get_date_range(cursor)
        unique_domains, domain_counts, domains_removed = extract_domains_from_urls(cursor, patterns, max_workers)
        
        logging.info("Closing database connection")
        conn.close()
        
        total_time = (time.time() - total_start_time) * 1000
        logging.info(f"Analysis completed successfully in {total_time:.1f}ms")
        return {
            'date_range': (earliest_date, latest_date, days_between),
            'unique_domains': unique_domains,
            'domain_counts': domain_counts,
            'total_unique_domains': len(unique_domains),
            'top_domains': domain_counts.most_common(top_domains),
            'domains_removed': domains_removed
        }
    
    finally:
        if os.path.exists(temp_history_path):
            logging.info("Cleaning up temporary database file")
            os.remove(temp_history_path)

def format_number(num):
    return f"{num:,}"

def redact_domain(domain):
    if not domain:
        return domain
    
    parts = domain.split('.')
    if len(parts) <= 1:
        return domain
    
    if len(parts) >= 2 and len(parts[-2]) <= 3:
        return f"???.{parts[-1]}"
    
    redacted_parts = ['*' * len(part) for part in parts[:-1]]
    redacted_parts.append(parts[-1])
    
    return '.'.join(redacted_parts)

def print_analysis_results(results, browser_name='Vivaldi', top_domains=None, bottom_domains=None, redact=False):
    earliest_date, latest_date, days_between = results['date_range']
    
    print(f"\n--- {browser_name} History Analysis ---")
    
    if days_between > 0:
        print(f"Date range: {earliest_date} to {latest_date} ({format_number(days_between)} days)")
    else:
        print(f"Date range: {earliest_date} to {latest_date}")
    
    print(f"Total unique domains found: {format_number(results['total_unique_domains'])}")
    print(f"Domains removed (no valid TLD): {format_number(results['domains_removed'])}")
    
    if top_domains is not None:
        print(f"\nTop {min(top_domains, len(results['top_domains']))} most visited domains:")
        for domain, count in results['top_domains'][:top_domains]:
            display_domain = redact_domain(domain) if redact else domain
            print(f"- {display_domain}: {format_number(count)} visits")
    
    if bottom_domains is not None and bottom_domains > 0:
        sorted_domains = sorted(results['domain_counts'].items(), key=lambda x: x[1])
        bottom_domains_list = sorted_domains[:bottom_domains]
        
        print(f"\nBottom {len(bottom_domains_list)} least visited domains:")
        for domain, count in bottom_domains_list:
            display_domain = redact_domain(domain) if redact else domain
            print(f"- {display_domain}: {format_number(count)} visits")

def create_parser():
    parser = argparse.ArgumentParser(
        description='Analyze browser history to find unique domains and their visit counts.',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s                           # Default analysis with top 10 domains
  %(prog)s --top 20                  # Show only top 20 domains
  %(prog)s --bottom 10               # Show only bottom 10 domains
  %(prog)s --top 20 --bottom 10      # Show both top 20 and bottom 10
  %(prog)s --browser Chrome          # Analyze Chrome instead of Vivaldi
  %(prog)s --patterns custom.txt     # Use custom pattern file
  %(prog)s --no-patterns             # Disable pattern normalization
  %(prog)s --temp-path /tmp/hist.db  # Use custom temporary file path
  %(prog)s --verbose                 # Enable verbose logging with timing
  %(prog)s --workers 4               # Use 4 worker threads for processing
  %(prog)s --redact                  # Redact domain names for privacy
        """
    )
    
    parser.add_argument('--browser', '-b', default='Vivaldi', help='Browser to analyze (default: Vivaldi)')
    parser.add_argument('--top', '-t', type=int, default=None, help='Number of top domains to display (default: 10 when no --bottom specified)')
    parser.add_argument('--bottom', '-bt', type=int, default=None, help='Number of bottom domains to display')
    parser.add_argument('--patterns', '-p', dest='pattern_file_path', help='Path to custom domain pattern file (default: domain_patterns.txt)')
    parser.add_argument('--no-patterns', action='store_true', help='Disable pattern-based domain normalization')
    parser.add_argument('--temp-path', help='Custom temporary file path for database copy')
    parser.add_argument('--quiet', '-q', action='store_true', help='Suppress warning messages')
    parser.add_argument('--verbose', '-v', action='store_true', help='Enable verbose logging with timing information')
    parser.add_argument('--workers', '-w', type=int, default=None, help='Number of worker threads (default: auto-detect, max 8)')
    parser.add_argument('--redact', action='store_true', help='Redact domain names for privacy (shows only TLD)')
    parser.add_argument('--version', action='version', version='%(prog)s 1.0')
    
    return parser

def main_cli():
    parser = create_parser()
    args = parser.parse_args()
    
    # Set up logging first
    setup_logging(args.verbose)
    
    if args.no_patterns:
        pattern_file_path = None
    elif args.pattern_file_path is not None:
        pattern_file_path = args.pattern_file_path
    else:
        # No pattern file specified and --no-patterns not used
        # Use default patterns (None means use defaults)
        pattern_file_path = None
    
    if args.top is not None and args.top < 0:
        print("Error: --top must be non-negative", file=sys.stderr)
        sys.exit(1)
    
    if args.bottom is not None and args.bottom < 0:
        print("Error: --bottom must be non-negative", file=sys.stderr)
        sys.exit(1)
    
    if args.workers is not None and args.workers < 1:
        print("Error: --workers must be at least 1", file=sys.stderr)
        sys.exit(1)
    
    show_top = args.top is not None
    show_bottom = args.bottom is not None
    
    if not show_top and not show_bottom:
        show_top = True
        top_count = 10
    else:
        top_count = args.top if show_top else 1
    
    if args.quiet:
        import warnings
        warnings.filterwarnings("ignore")
    
    try:
        results = analyze_browser_history(
            browser_name=args.browser,
            temp_path=args.temp_path,
            top_domains=top_count,
            pattern_file_path=pattern_file_path,
            max_workers=args.workers
        )
        
        print_analysis_results(
            results, 
            browser_name=args.browser,
            top_domains=top_count if show_top else None,
            bottom_domains=args.bottom if show_bottom else None,
            redact=args.redact
        )
        
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)

def main(browser_name='Vivaldi', top_domains=10, temp_path=None, pattern_file_path=None, redact=False):
    try:
        results = analyze_browser_history(
            browser_name=browser_name,
            temp_path=temp_path,
            top_domains=top_domains,
            pattern_file_path=pattern_file_path
        )
        print_analysis_results(results, browser_name, redact=redact)
        
    except Exception as e:
        print(f"An error occurred: {e}")

if __name__ == "__main__":
    main_cli()
