#!/bin/sh

echo "Running filesystem tests..."

# 创建测试目录
mkdir test_dir
echo "Created test_dir"

# 进入测试目录
cd test_dir

# 创建文件并写入内容
echo "Hello, AlienFS!" > test_file.txt
echo "Created and wrote to test_file.txt"

# 读取文件内容
cat test_file.txt

# 追加内容
echo "Appending some data..." >> test_file.txt
cat test_file.txt

# 创建子目录
mkdir sub_dir
echo "Created sub_dir"

# 移动文件到子目录
mv test_file.txt sub_dir/
echo "Moved test_file.txt to sub_dir"

# 删除文件
rm sub_dir/test_file.txt
echo "Deleted test_file.txt"

# 删除子目录
rmdir sub_dir
echo "Deleted sub_dir"

# 返回上级目录并删除测试目录
cd ..
rmdir test_dir
echo "Deleted test_dir"

echo "Filesystem tests completed!"
