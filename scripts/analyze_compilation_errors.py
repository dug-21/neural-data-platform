#!/usr/bin/env python3
"""
Analyze compilation errors from cargo check output to identify patterns
and create a parallelization strategy for fixing.
"""

import re
from collections import defaultdict, Counter
from typing import Dict, List, Tuple
import json

class CompilationErrorAnalyzer:
    def __init__(self, log_file: str):
        self.log_file = log_file
        self.errors = []
        self.warnings = []
        self.file_errors = defaultdict(list)
        self.file_warnings = defaultdict(list)
        self.error_types = Counter()
        self.warning_types = Counter()
        
    def parse_log(self):
        """Parse the compilation log and categorize errors/warnings."""
        with open(self.log_file, 'r') as f:
            content = f.read()
            
        # Regular expressions for parsing
        error_pattern = r'error\[([A-Z0-9]+)\]: (.+?)\n\s+-->\s+(.+?):(\d+):(\d+)'
        warning_pattern = r'warning: (.+?)\n\s+-->\s+(.+?):(\d+):(\d+)'
        
        # Find all errors
        for match in re.finditer(error_pattern, content, re.MULTILINE):
            error_code = match.group(1)
            error_msg = match.group(2)
            file_path = match.group(3)
            line_num = match.group(4)
            col_num = match.group(5)
            
            error_info = {
                'code': error_code,
                'message': error_msg,
                'line': int(line_num),
                'column': int(col_num),
                'file': file_path
            }
            
            self.errors.append(error_info)
            self.file_errors[file_path].append(error_info)
            self.error_types[error_code] += 1
            
        # Find all warnings
        for match in re.finditer(warning_pattern, content, re.MULTILINE):
            warning_msg = match.group(1)
            file_path = match.group(2)
            line_num = match.group(3)
            col_num = match.group(4)
            
            # Categorize warning type
            warning_type = 'unknown'
            if 'unused import' in warning_msg:
                warning_type = 'unused_import'
            elif 'unused variable' in warning_msg:
                warning_type = 'unused_variable'
            elif 'never read' in warning_msg:
                warning_type = 'field_never_read'
            elif 'never used' in warning_msg:
                warning_type = 'never_used'
            elif 'deprecated' in warning_msg:
                warning_type = 'deprecated'
            elif 'does not need to be mutable' in warning_msg:
                warning_type = 'unnecessary_mut'
                
            warning_info = {
                'type': warning_type,
                'message': warning_msg,
                'line': int(line_num),
                'column': int(col_num),
                'file': file_path
            }
            
            self.warnings.append(warning_info)
            self.file_warnings[file_path].append(warning_info)
            self.warning_types[warning_type] += 1
            
    def analyze_patterns(self) -> Dict:
        """Analyze patterns in errors and warnings."""
        patterns = {
            'total_errors': len(self.errors),
            'total_warnings': len(self.warnings),
            'files_with_errors': len(self.file_errors),
            'files_with_warnings': len(self.file_warnings),
            'error_type_distribution': dict(self.error_types),
            'warning_type_distribution': dict(self.warning_types),
            'common_patterns': []
        }
        
        # Identify common patterns
        if 'E0432' in self.error_types:
            patterns['common_patterns'].append({
                'pattern': 'Missing AdapterMetadata import',
                'count': self.error_types['E0432'],
                'files': [e['file'] for e in self.errors if e['code'] == 'E0432']
            })
            
        if 'E0277' in self.error_types:
            patterns['common_patterns'].append({
                'pattern': 'NeuralConfig Builder trait issues',
                'count': self.error_types['E0277'],
                'files': [e['file'] for e in self.errors if e['code'] == 'E0277']
            })
            
        return patterns
        
    def create_fix_strategy(self) -> List[Dict]:
        """Create a parallelization strategy for fixing errors."""
        strategy = []
        
        # Group 1: Import fixes (can be done in parallel)
        import_fixes = []
        for file, errors in self.file_errors.items():
            import_errors = [e for e in errors if e['code'] == 'E0432']
            if import_errors:
                import_fixes.append({
                    'file': file,
                    'fixes': ['Add AdapterMetadata to adapters/mod.rs exports']
                })
                
        if import_fixes:
            strategy.append({
                'group': 'Import Fixes',
                'parallel': True,
                'priority': 1,
                'files': import_fixes
            })
            
        # Group 2: Builder pattern fixes
        builder_fixes = []
        for file, errors in self.file_errors.items():
            builder_errors = [e for e in errors if e['code'] == 'E0277']
            if builder_errors:
                builder_fixes.append({
                    'file': file,
                    'fixes': ['Add #[builder(default)] to NeuralConfig fields']
                })
                
        if builder_fixes:
            strategy.append({
                'group': 'Builder Pattern Fixes',
                'parallel': True,
                'priority': 2,
                'files': builder_fixes
            })
            
        # Group 3: Module visibility fixes
        visibility_fixes = []
        for file, errors in self.file_errors.items():
            vis_errors = [e for e in errors if e['code'] == 'E0603']
            if vis_errors:
                visibility_fixes.append({
                    'file': file,
                    'fixes': ['Fix module visibility in vendor dependencies']
                })
                
        if visibility_fixes:
            strategy.append({
                'group': 'Visibility Fixes',
                'parallel': False,  # May have dependencies
                'priority': 3,
                'files': visibility_fixes
            })
            
        # Group 4: Logic fixes (method not found, type mismatches)
        logic_fixes = []
        for file, errors in self.file_errors.items():
            logic_errors = [e for e in errors if e['code'] in ['E0599', 'E0308', 'E0382']]
            if logic_errors:
                logic_fixes.append({
                    'file': file,
                    'fixes': ['Fix method implementations and type mismatches']
                })
                
        if logic_fixes:
            strategy.append({
                'group': 'Logic Fixes',
                'parallel': False,  # Requires careful handling
                'priority': 4,
                'files': logic_fixes
            })
            
        # Group 5: Cleanup warnings (can be done in parallel)
        warning_fixes = []
        for file, warnings in self.file_warnings.items():
            if warnings:
                fixes = []
                unused_imports = [w for w in warnings if w['type'] == 'unused_import']
                if unused_imports:
                    fixes.append(f'Remove {len(unused_imports)} unused imports')
                    
                unused_vars = [w for w in warnings if w['type'] == 'unused_variable']
                if unused_vars:
                    fixes.append(f'Prefix {len(unused_vars)} unused variables with _')
                    
                if fixes:
                    warning_fixes.append({
                        'file': file,
                        'fixes': fixes
                    })
                    
        if warning_fixes:
            strategy.append({
                'group': 'Warning Cleanup',
                'parallel': True,
                'priority': 5,
                'files': warning_fixes[:10]  # Limit to avoid too many parallel operations
            })
            
        return strategy
        
    def generate_report(self) -> str:
        """Generate a comprehensive error analysis report."""
        patterns = self.analyze_patterns()
        strategy = self.create_fix_strategy()
        
        report = f"""# Compilation Error Analysis Report

## Summary
- **Total Errors**: {patterns['total_errors']}
- **Total Warnings**: {patterns['total_warnings']}
- **Files with Errors**: {patterns['files_with_errors']}
- **Files with Warnings**: {patterns['files_with_warnings']}

## Error Distribution
"""
        for error_type, count in sorted(patterns['error_type_distribution'].items(), key=lambda x: x[1], reverse=True):
            report += f"- **{error_type}**: {count} occurrences\n"
            
        report += "\n## Warning Distribution\n"
        for warning_type, count in sorted(patterns['warning_type_distribution'].items(), key=lambda x: x[1], reverse=True):
            report += f"- **{warning_type}**: {count} occurrences\n"
            
        report += "\n## Common Patterns\n"
        for pattern in patterns['common_patterns']:
            report += f"\n### {pattern['pattern']}\n"
            report += f"- Count: {pattern['count']}\n"
            report += f"- Files: {', '.join(pattern['files'])}\n"
            
        report += "\n## Files with Most Errors\n"
        file_error_counts = [(f, len(e)) for f, e in self.file_errors.items()]
        for file, count in sorted(file_error_counts, key=lambda x: x[1], reverse=True)[:10]:
            report += f"- **{file}**: {count} errors\n"
            
        report += "\n## Parallelization Strategy\n"
        for group in strategy:
            report += f"\n### {group['group']} (Priority {group['priority']})\n"
            report += f"- **Can Parallelize**: {'Yes' if group['parallel'] else 'No'}\n"
            report += f"- **Files to Fix** ({len(group['files'])}):\n"
            for file_info in group['files'][:5]:  # Show first 5
                report += f"  - {file_info['file']}\n"
                for fix in file_info['fixes']:
                    report += f"    - {fix}\n"
            if len(group['files']) > 5:
                report += f"  - ... and {len(group['files']) - 5} more files\n"
                
        report += "\n## Recommended Fix Order\n"
        report += """
1. **Phase 1 (Parallel)**: Import fixes - Add AdapterMetadata to exports
2. **Phase 2 (Parallel)**: Builder pattern - Add default derives
3. **Phase 3 (Sequential)**: Module visibility - Fix vendor dependencies
4. **Phase 4 (Sequential)**: Logic fixes - Method implementations
5. **Phase 5 (Parallel)**: Warning cleanup - Remove unused code

**Note**: Phases 1, 2, and 5 can be executed in parallel across different files
to avoid locking issues.
"""
        
        return report


if __name__ == "__main__":
    analyzer = CompilationErrorAnalyzer("/workspaces/neural-trader/compilation_errors.log")
    analyzer.parse_log()
    
    # Generate and save report
    report = analyzer.generate_report()
    with open("/workspaces/neural-trader/COMPILATION_ERROR_CATALOG.md", "w") as f:
        f.write(report)
        
    # Save detailed JSON data
    detailed_data = {
        'errors': analyzer.errors,
        'warnings': analyzer.warnings,
        'file_errors': dict(analyzer.file_errors),
        'file_warnings': dict(analyzer.file_warnings),
        'strategy': analyzer.create_fix_strategy()
    }
    
    with open("/workspaces/neural-trader/compilation_errors_detailed.json", "w") as f:
        json.dump(detailed_data, f, indent=2)
        
    print("Analysis complete!")
    print(f"Total errors: {len(analyzer.errors)}")
    print(f"Total warnings: {len(analyzer.warnings)}")
    print("\nReport saved to: COMPILATION_ERROR_CATALOG.md")
    print("Detailed data saved to: compilation_errors_detailed.json")